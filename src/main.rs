extern crate inotify;
extern crate lettre;
extern crate lettre_email;

use inotify::{
	EventMask,
	WatchMask,
	Inotify,
};

use lettre::{SmtpClient, Transport};
use lettre::smtp::authentication::{Credentials, Mechanism};
use lettre::smtp::extension::ClientId;
use lettre::smtp::ConnectionReuseParameters;
use lettre_email::Email;

use std::io::SeekFrom;
use std::io::prelude::*;
use std::fs::File;
use std::os::linux::fs::MetadataExt;
use std::path::Path;
use std::error;
use std::fmt;

use serde::Deserialize;

//Errors ------------------------------------------------

#[derive(Debug, Clone)]
struct ParentPathError;

impl fmt::Display for ParentPathError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "No parent folder found for given path")
    }
}

impl error::Error for ParentPathError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

#[derive(Debug, Clone)]
struct InitError;

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Could not initialize subscription")
    }
}

impl error::Error for InitError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl std::convert::From<std::io::Error> for InitError {
	fn from(_error: std::io::Error) -> Self {
        InitError
    }
}

//Handlers ------------------------------------------------

fn handle_line(config : &Config, line_str: &str) {
	println!("Line: {}", &line_str);
	if line_str.contains("Accepted password") {
		println!("ACCEPTED PASSWORD!!!\n!\n!\n!\n!\n");
		for recipient in &config.recipients {
			let email = Email::builder()
				.to((recipient.email.to_string(), recipient.name.to_string()))
				.from(config.sender.email.to_string())
				.subject("Automated message: SSH Login")
				.text(
					"".to_owned() +
					"The following SSH login was recorded:\n" +
					line_str +
					"\n" +
					"On the following machine:\n" +
					"Hostname: " + &sys_info::hostname().unwrap_or("UNKNOWN".to_string()) + "\n" +
					"OS release: " + &sys_info::os_release().unwrap_or("UNKNOWN".to_string()) + "\n" +
					"OS type: " +  &sys_info::os_type().unwrap_or("UNKNOWN".to_string()) + "\n"
				)
				.build()
				.unwrap();

			// Open a local connection on port 25
			let mut mailer = SmtpClient::new_simple(&config.sender.smtp_server).unwrap()
				// Set the name sent during EHLO/HELO, default is `localhost`
				.hello_name(ClientId::Domain("localhost".to_string()))
				// Add credentials for authentication
				.credentials(Credentials::new(config.sender.email.to_string(), config.sender.password.to_string()))
				// Enable SMTPUTF8 if the server supports it
				.smtp_utf8(true)
				// Configure expected authentication mechanism
				.authentication_mechanism(Mechanism::Plain)
				// Enable connection reuse
				.connection_reuse(ConnectionReuseParameters::ReuseUnlimited).transport();
			
			// Send the email
			let result = mailer.send(email.into());

			if result.is_ok() {
				println!("Email sent");
			} else {
				println!("Could not send email: {:?}", result);
			}

			assert!(result.is_ok());
		}
	}
}

fn handle_bytes(config : &Config, reader : &mut LogReader, mut read_buf: &[u8]) {
	//println!("Contents: [{:?}]", &read_buf);
	//println!("Contents: [{}]", str::from_utf8_lossy(&read_buf));
	loop {
		match read_buf.iter().position(|&r| r == '\n' as u8){
			Some(newline_idx) => {
				//println!("Newline matched: {}", newline_idx);
				reader.buf_vec.extend_from_slice(&read_buf[0..newline_idx+1]);
				handle_line(config, &String::from_utf8_lossy(&reader.buf_vec));
				reader.buf_vec.clear();
				read_buf = &read_buf[newline_idx+1..] //handle rest of the buffer
			},
			None => {
				//println!("None matched.");
				reader.buf_vec.extend_from_slice(read_buf);
				break;
			},
		}
	}
}

//---------------------------------------------------

fn read_once(config : &Config, subscription : &mut LogSubscription) -> Result<(), std::io::Error> {
	if let Some(ref mut f) = subscription.file_handle {
		loop {
			const BUF_SZ: usize = 4096;
			let mut buf = [0u8; BUF_SZ];
			let bytes_read = f.read(&mut buf)?;
			if bytes_read > 0 {
				println!("Bytes read: {}", bytes_read);
				handle_bytes(config, &mut subscription.log_reader, &mut buf[0..bytes_read]);
			}
			if bytes_read < BUF_SZ { //Buffer not exhausted, read everything
				let curr_pos = f.seek(SeekFrom::Current(0))?;
				println!("Seek pos: {}", curr_pos);
				let end_pos = f.seek(SeekFrom::End(0))?;
				// Almost always, it holds that (end_pos == curr_pos)
				if end_pos < curr_pos {
					// File was truncated
					println!("File was truncated!!");
					f.seek(SeekFrom::Start(0))?;
					continue;
				}else if end_pos > curr_pos {
					// Someone has appended to the file in the meantime, reset cursor
					f.seek(SeekFrom::Start(curr_pos))?;
					// Do nothing because we should have
					// received another inotify event for that
				}
				break;
			}
		}
		Ok(())
	} else {
		Err(std::io::Error::new(std::io::ErrorKind::Other, "File handle does not exist!"))
	}
}

//---------------------------------------------------

fn event_handler(config : &Config, inotify : &mut Inotify, subscription : &mut LogSubscription, event : &inotify::Event<&std::ffi::OsStr>) {
	if let Some(ref mut f) = subscription.file_handle {
		if event.mask.contains(EventMask::MODIFY) {
			read_once(config, subscription);
		} else if event.mask.contains(EventMask::ATTRIB) {
			//TODO: Remove .expect() here because this error can happen during normal operation
			let num_links = f.metadata().expect("Could not read file metadata").st_nlink();
			if num_links == 0 {
				println!("File was deleted!");
				subscription.log_reader.buf_vec.clear(); // Reset buffer (clear old, bytes)
				subscription.file_handle = None;
			}else{
				println!("Just attrib change!");
			}
		} else if event.mask.contains(EventMask::MOVE_SELF) {
			subscription.file_handle = None;
			println!("File was moved!");
		}
	}
	
	if let Some(ref name) = event.name{
		if name == &subscription.file_name.as_os_str() {
			if event.mask.contains(EventMask::CREATE) || event.mask.contains(EventMask::MOVED_TO) {
				if let Ok(_) = init_reader(inotify, subscription, true) {
					read_once(config, subscription);
				}
				// If we cannot init the reader again, we do not read and ignore the error
				// May happen if file is created / moved and then quickly deleted / unmounted
				// We will recover when file is created / moved again
			}else if event.mask.contains(EventMask::DELETE) {
				println!("File was deleted (from the outside)");
				subscription.log_reader.buf_vec.clear(); // Reset buffer (clear old, bytes)
				subscription.file_handle = None;
			}
		}
	}
	
}

//---------------------------------------------------

struct LogReader {
	buf_vec: Vec<u8>
}

struct LogSubscription {
	file_name: std::ffi::OsString,
    file_path: std::path::PathBuf,
    file_watch: Option<inotify::WatchDescriptor>,
	dir_path: std::path::PathBuf,
    dir_watch: Option<inotify::WatchDescriptor>,
	file_handle: Option<std::fs::File>,
	log_reader: LogReader,
}

impl LogSubscription {
    pub fn new(logfile: &std::path::Path) -> Result<LogSubscription, ParentPathError> {
		let parent = logfile.parent().ok_or(ParentPathError)?;
		let file = logfile.file_name().ok_or(ParentPathError)?;
		Ok(LogSubscription {
			file_name: file.to_os_string(),
			file_path: logfile.to_path_buf(),
			file_watch: None,
			dir_path: parent.to_path_buf(),
			dir_watch: None,
			file_handle: None,
			log_reader: LogReader {
				buf_vec: Vec::new(),
			},
		})
    }
}

//---------------------------------------------------

fn init_reader(inotify : &mut Inotify, subscription : &mut LogSubscription, from_beginning : bool) -> Result<(), InitError>{
	subscription.file_watch = Some(inotify
		.add_watch(
			&subscription.file_path,
			WatchMask::MODIFY | WatchMask::ATTRIB | WatchMask::MOVE_SELF,
		)?);
	
	subscription.file_handle = Some(File::open(&subscription.file_path)?);
	if !from_beginning {
		if let Some(ref mut f) = subscription.file_handle {
			f.seek(SeekFrom::End(0))?; // Go to end of file
		}
	}
	println!("File was opened successfully!");
	Ok(())
}

//---------------------------------------------------

#[derive(Deserialize)]
struct Config {
    sender: Sender,
    recipients: Vec<Recipient>,
}

#[derive(Deserialize)]
struct Sender {
    email: String,
	password: String,
	smtp_server: String,
}

#[derive(Deserialize)]
struct Recipient {
    email: String,
    name: String,
}

//---------------------------------------------------

fn main() {
	let monitored_path = "/mnt/d/Misc/Projects/Rust/monboi/testfile.txt";//"/home/test/Desktop/inotifyimpl/foo.txt"
	let config_path = "/mnt/d/Misc/Projects/Rust/monboi/monboi.conf";//"/etc/monboi/monboi.conf"
	let mut config_file = File::open(&config_path).unwrap();
	let mut config_toml = String::new();
	config_file.read_to_string(&mut config_toml).unwrap();
	
	let config: Config = toml::from_str(&config_toml).unwrap();
	
	let mut inotify = Inotify::init()
		.expect("Failed to initialize inotify");
	
	let mut subscription_1 = LogSubscription::new(Path::new(monitored_path)).expect("Could not create subscription!");
	subscription_1.dir_watch = Some(inotify
		.add_watch(
			&subscription_1.dir_path,
			WatchMask::CREATE | WatchMask::MOVED_TO | WatchMask::DELETE,
		)
		.expect("Failed to add inotify watch"));
	
	let _ = init_reader(&mut inotify, &mut subscription_1, false);

	let mut inotify_buf = [0u8; 4096];
	loop {
		let events = inotify
			.read_events_blocking(&mut inotify_buf)
			.expect("Failed to read inotify events");
		
		for event in events {
			println!("Handle event: {:?}", event);
			event_handler(&config, &mut inotify, &mut subscription_1, &event);
		}
	}
}

//---------------------------------------------------

//WatchMask::MODIFY
// File was deleted or moved, reopen file handle and read entire file again
//WatchMask::DELETE (ONLY ON DIREFTORY)
//WatchMask::ATTRIB (on file itself - NEED TO CHECK f.metadata().expect("Could not read metadata!").st_nlink() to see if file was unlinked!!)
//WatchMask::MOVED_FROM (on file itself)
//WatchMask::MOVED_TO (on file itself)
//WatchMask::MOVE_SELF (on file itself)
//Listen to all:
//WatchMask::ALL_EVENTS
//Event { wd: WatchDescriptor { id: 1, fd: (Weak) }, mask: MOVED_FROM, cookie: 801, name: Some("foo.txt") }
//Event { wd: WatchDescriptor { id: 1, fd: (Weak) }, mask: MOVED_TO, cookie: 801, name: Some("foo2.txt") }
//Event { wd: WatchDescriptor { id: 2, fd: (Weak) }, mask: MOVE_SELF, cookie: 0, name: None }

//---------------------------------------------------

// Attempt to load and parse the config file into our Config struct.
// If a file cannot be found, return a default Config.
// If we find a file but cannot parse it, panic
/*pub fn parse(path: String) -> Config {
    let mut config_toml = String::new();

    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(_)  => {
            println!("Could not find config file, using default!");
            return Config::new();
        }
    };

    file.read_to_string(&mut config_toml)
            .unwrap_or_else(|err| panic!("Error while reading config: [{}]", err));

    let mut parser = Parser::new(&config_toml);
    let toml = parser.parse();

    if toml.is_none() {
        for err in &parser.errors {
            let (loline, locol) = parser.to_linecol(err.lo);
            let (hiline, hicol) = parser.to_linecol(err.hi);
            println!("{}:{}:{}-{}:{} error: {}",
                     path, loline, locol, hiline, hicol, err.desc);
        }
        panic!("Exiting server");
    }

    let config = Value::Table(toml.unwrap());
    match toml::decode(config) {
        Some(t) => t,
        None => panic!("Error while deserializing config")
    }
}*/
/*let config: Config = toml::from_str(r#"
        [sender]
		email = 'sefsefsef'
		password = 'sefsefsef'
		smtp_server = 'g43t45t'

        [[recipients]]
        email = 'xxxxxxxxxxxxxxxxx'
        name = 'yyyyyyyyyyyyyyyyy'
    "#).unwrap();
*/

