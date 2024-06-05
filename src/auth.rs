use crate::errors::LoginError;
use crate::utils::{
    define_security_file_location,
    read_file,
};
use email_address_parser::{
    EmailAddress,
    ParsingOptions,
};
use reqwest::Client;
use rpassword::read_password;
use serde_derive::{
    Deserialize,
    Serialize,
};
use std::{
    fs::OpenOptions,
    io::{
        self,
        Write,
    },
};
use yansi::Paint;

#[derive(Debug, Serialize, Deserialize)]
pub struct Login {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub status: String,
    pub token: String,
}

pub async fn login() -> Result<(), LoginError> {
    print!("ℹ️  If you do not have an account, please go to soldeer.xyz to create one.\n📧 Please enter your email: ");
    std::io::stdout().flush().unwrap();
    let mut email = String::new();
    if io::stdin().read_line(&mut email).is_err() {
        return Err(LoginError {
            cause: "Invalid email".to_string(),
        });
    }
    email = match check_email(email) {
        Ok(e) => e,
        Err(err) => return Err(err),
    };

    print!("🔓 Please enter your password: ");
    std::io::stdout().flush().unwrap();
    let password = read_password().unwrap();

    let login: Login = Login { email, password };

    execute_login(login).await.unwrap();
    Ok(())
}

pub fn get_token() -> Result<String, LoginError> {
    let security_file = define_security_file_location();
    let jwt = read_file(security_file);
    match jwt {
        Ok(token) => {
            Ok(String::from_utf8(token)
                .expect("You are not logged in. Please login using the 'soldeer login' command"))
        }
        Err(_) => {
            Err(LoginError {
                cause: "You are not logged in. Please login using the 'login' command".to_string(),
            })
        }
    }
}

fn check_email(email_str: String) -> Result<String, LoginError> {
    let email_str = email_str.trim().to_string().to_ascii_lowercase();

    let email: Option<EmailAddress> =
        EmailAddress::parse(&email_str, Some(ParsingOptions::default()));
    if email.is_none() {
        Err(LoginError {
            cause: "Invalid email".to_string(),
        })
    } else {
        Ok(email_str)
    }
}

async fn execute_login(login: Login) -> Result<(), LoginError> {
    let url = format!("{}/api/v1/auth/login", crate::BASE_URL);
    let req = Client::new().post(url).json(&login);

    let login_response = req.send().await;

    let security_file = define_security_file_location();
    if let Ok(response) = login_response {
        if response.status().is_success() {
            println!("{}", Paint::green("Login successful"));
            // print!("{:}", &response.text().await.unwrap());
            let jwt = serde_json::from_str::<LoginResponse>(&response.text().await.unwrap())
                .unwrap()
                .token;
            let mut file: std::fs::File = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .append(false)
                .open(&security_file)
                .unwrap();
            if let Err(err) = write!(file, "{}", &jwt) {
                return Err(LoginError {
                    cause: format!(
                        "Couldn't write to the security file {}: {}",
                        &security_file, err
                    ),
                });
            }
            println!(
                "{}",
                Paint::green(&format!("Login details saved in: {:?}", &security_file))
            );

            return Ok(());
        } else if response.status().as_u16() == 401 {
            return Err(LoginError {
                cause: "Authentication failed. Invalid email or password".to_string(),
            });
        }
    }

    Err(LoginError {
        cause: "Authentication failed. Unknown error.".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs::remove_file,
        path::Path,
        process::exit,
    };

    use serial_test::serial;

    use crate::utils::read_file_to_string;

    use super::*;

    #[test]
    #[serial]
    fn email_validation() {
        let valid_email = String::from("test@test.com");
        let invalid_email = String::from("test");

        assert_eq!(check_email(valid_email.clone()).unwrap(), valid_email);

        let expected_error = LoginError {
            cause: "Invalid email".to_string(),
        };
        assert_eq!(check_email(invalid_email).err().unwrap(), expected_error);
    }

    #[tokio::test]
    #[serial]
    async fn login_success() {
        let opts = mockito::ServerOpts {
            host: "0.0.0.0",
            port: 1234,
            ..Default::default()
        };
        let data = r#"
        {
            "status": "200",
            "token": "jwt_token_example"
        }"#;

        // Request a new server from the pool
        let mut server = mockito::Server::new_with_opts_async(opts).await;

        // Use one of these addresses to configure your client

        // Create a mock
        let _ = server
            .mock("POST", "/api/v1/auth/login")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(data)
            .create();

        match execute_login(Login {
            email: "test@test.com".to_string(),
            password: "1234".to_string(),
        })
        .await
        {
            Ok(_) => {
                let results = read_file_to_string(&"./test_save_jwt".to_string());
                assert_eq!(results, "jwt_token_example");
                let _ = remove_file("./test_save_jwt");
                return ();
            }
            Err(_) => exit(1),
        };
    }
}
