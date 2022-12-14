use rand::distributions::{Alphanumeric, Uniform};
use rand::seq::index::sample;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::borrow::{Borrow, Cow};
use std::collections::HashMap;

use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::pbes::EncryptionScheme;
use ron::ser;

//use der::Document;

//use pkcs5::der::{Decode, Encode};
//use pkcs5::{pbes2::Parameters, EncryptionScheme};

/* Type declaration for entry of a password */
pub type PasswordEntries<'a> = HashMap<Cow<'a, str>, Password<'a>>;

/* Home directory path depending on OS. Used to create pwmanager directory */
pub const HOME_ENV: &str = if cfg!(windows) {
    "USERPROFILE"
} else if cfg!(unix) {
    "HOME"
} else {
    "NONEXISTANT"
};

pub fn add_password_32<'a>(
    entries: &mut PasswordEntries<'a>,
    name: &'a str,
) -> Option<Password<'a>> {
    let pw: Password = Password::new_password32();
    entries.insert(Cow::from(name), pw)
}

pub fn add_password_64<'a>(
    entries: &mut PasswordEntries<'a>,
    name: &'a str,
) -> Option<Password<'a>> {
    let pw: Password = Password::new_password64();
    entries.insert(Cow::from(name), pw)
}

pub fn write_to_file(file: Option<&str>, entries: &PasswordEntries) -> io::Result<()> {
    let f = File::create(file.unwrap_or("passwords.json"))?;
    serde_json::to_writer(f, entries)?;
    Ok(())
}

pub fn read_from_file<'a, 'b>(file: Option<&'b str>) -> io::Result<PasswordEntries<'a>> {
    let file = File::open(file.unwrap_or("passwords.json"))?;
    let pw_entry: PasswordEntries = serde_json::from_reader(&file)?;
    return Ok(pw_entry);
}

/* Struct for list of modules */
pub struct ModuleList<'a> {
    pub modules: Vec<(Cow<'a, str>, Option<PasswordEntries<'a>>)>,
    pub encryptions: HashMap<Cow<'a, str>, EncryptionScheme<'a>>,
}

impl<'b> ModuleList<'b> {
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
            encryptions: HashMap::new(),
        }
    }

    pub fn add_module(
        &mut self,
        name: &str,
        entries: PasswordEntries<'b>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut base_path = PathBuf::from(env::var(HOME_ENV)?);
        let file_name = format!("{}.json", name);
        let file_name = base_path.join(file_name);
        return match File::open(&file_name) {
            Ok(_) => Err(Box::new(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Module already exists.",
            ))),
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    self.modules
                        .push((Cow::Owned(name.to_owned()), Some(entries)));
                    Ok(())
                } else {
                    Err(Box::new(e))
                }
            }
        };
    }

    pub fn write_module(
        name: &str,
        entries: &PasswordEntries,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut base_path = PathBuf::from(env::var(HOME_ENV)?);
        base_path.push(".pwmanager");

        let file_name = format!("{}.json", name);
        let file_name = base_path.join(file_name);
        let f = File::create(&file_name)?;
        serde_json::to_writer(f, entries)?;
        Ok(())
    }

    pub fn encrypt_module<'a: 'b>(
        &mut self,
        entry: &mut (Cow<'a, str>, Option<PasswordEntries<'a>>),
        password: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(_) = self.encryptions.get(&entry.0) {
            return Ok(());
        }
        let mut base_path = PathBuf::from(env::var(HOME_ENV)?);
        base_path.push(".pwmanager");

        let file_name = format!("{}.json", &entry.0);
        let file_name = base_path.join(file_name);
        let ec = encrypt_file(password, &file_name.to_string_lossy())?;
        self.encryptions.insert(entry.0.to_owned(), ec);
        Ok(())
    }

    pub fn get_module_list(enc: Option<&'b Vec<u8>>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut base_path = PathBuf::from(env::var(HOME_ENV)?);
        base_path.push(".pwmanager");
        let mut mod_list = Self::new();

        let mut entry_iter = base_path.read_dir()?;
        for entry in entry_iter.by_ref() {
            if let Ok(p) = entry {
                let path = p.path();
                let mod_name = path
                    .file_name()
                    .and_then(|s| Path::new(s).file_stem())
                    .unwrap()
                    .to_string_lossy();
                let extension = path.extension().unwrap();
                if extension == "json" {
                    mod_list
                        .modules
                        .push((Cow::from(mod_name.into_owned()), None));
                }
                if let Some(content) = enc {
                    mod_list.encryptions =
                        ron::de::from_bytes(content).expect("FAILED GETTING LIST");
                }
            }
        }
        Ok(mod_list)
    }

    pub fn get_encryptions(&mut self) {}
}

/* Struct to hold a password */
#[derive(Serialize, Deserialize, Debug)]
pub struct Password<'a>(pub Cow<'a, str>);

impl Password<'_> {
    /* Creates a password from the given password*/
    pub fn new_from(password: &str) -> Self {
        Self(Cow::Owned(password.to_owned()))
    }
    /* Generate random 32 byte password */
    pub fn new_password32() -> Self {
        Self(Cow::from(Self::generate_random_string(32)))
    }
    /* Generate a random 64 byte password*/
    pub fn new_password64() -> Self {
        Self(Cow::from(Self::generate_random_string(64)))
    }
    /* Generates a random string with a guaranteed special character and capital letter of length
     * len*/
    pub fn generate_random_string(len: usize) -> String {
        let mut rng = thread_rng();
        let spec_char: u8 = rng.sample(Uniform::new(33, 47));
        let cap_char: u8 = rng.sample(Uniform::new(65, 91));
        let ind = sample(&mut rng, len, 2);
        let pw: String = (&mut rng)
            .sample_iter(Alphanumeric)
            .take(len)
            .enumerate()
            .map(|(i, c)| {
                if (i == ind.index(0)) {
                    spec_char
                } else if (i == ind.index(1)) {
                    cap_char
                } else {
                    c
                }
            })
            .map(char::from)
            .collect();

        return pw;
    }
    pub fn get(&self) -> &str {
        &*self.0
    }

    pub fn encrypt_with_password<'a>(
        &self,
        file: &str,
    ) -> Result<EncryptionScheme<'a>, Box<dyn std::error::Error>> {
        encrypt_file(self.get(), file)
    }
}
/* Creates an encryption scheme and saves to a file with name file */
pub fn create_and_save_to_file(file: &str) -> Result<(), Box<dyn std::error::Error>> {
    unimplemented!()
}

/* Encrypt file using the given password, salt and iv. Return the resulting encryption scheme */
pub fn encrypt_file<'a>(
    password: &str,
    file: &str,
) -> Result<EncryptionScheme<'a>, Box<dyn std::error::Error>> {
    let ec = EncryptionScheme::default();
    let mut f = File::open(file)?;
    let mut content = Vec::new();
    f.read_to_end(&mut content)?;
    let encrypted_content = ec.encrypt(password, &content, file.as_bytes())?;

    let mut f = File::create(file)?;
    f.write_all(&encrypted_content)?;
    Ok(ec)
}

pub fn save_to_file<'a>(
    file: &str,
    ec_scheme: &EncryptionScheme<'a>,
) -> Result<(), Box<dyn std::error::Error>> {
    let f = File::create(file)?;
    ser::to_writer(f, ec_scheme)?;
    Ok(())
}

pub fn decrypt_file<'a>(
    password: &str,
    file: &str,
    ec_scheme: &EncryptionScheme<'a>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut encrypted_content: Vec<u8> = Vec::new();
    let mut f = File::open(file)?;
    f.read_to_end(&mut encrypted_content)?;
    let decrypted_content = ec_scheme.decrypt(password, &encrypted_content, file.as_bytes())?;

    let mut f = File::create(file)?;
    f.write_all(&decrypted_content)?;
    Ok(())
}

/* Encrypt file with password, hash it and save scheme to a file*/
pub fn password_encrypt_file(
    password: &str,
    file: &str,
    der_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let ec = encrypt_file(password, file)?;
    save_to_file(file, &ec)?;
    Ok(())
}

/* Encrypt file with password using the scheme and hash from der_file */

/* Decrypt file with password and scheme from der file */

pub fn decrypt_from_file(
    password: &str,
    file: &str,
    scheme_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut f = File::open(scheme_file)?;
    let mut content: Vec<u8> = Vec::new();
    f.read_to_end(&mut content)?;
    let ec: EncryptionScheme = ron::de::from_bytes(&content)?;
    decrypt_file(password, file, &ec)?;
    Ok(())
}
