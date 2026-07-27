// src/commands/platform.rs — platform:login

use crate::api::ApiClient;
use crate::parser::ParsedCommand;
use crate::session;
use crate::ui;

pub fn login(parsed: &ParsedCommand, client: &ApiClient) -> Result<(), String> {
    let email = parsed.get_str("email")
        .ok_or("Missing --email argument")?;
    let password = parsed.get_str("password")
        .ok_or("Missing --password argument")?;

    let spinner = ui::Spinner::start("Authenticating");
    let result  = client.login(email, password)?;

    let token = result["accessToken"].as_str()
        .ok_or("Login response missing accessToken")?
        .to_string();

    let user_email = result["user"]["email"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| email.to_string());

    let role = result["user"]["role"]
        .as_str()
        .map(|s| s.to_string());

    session::set(token, Some(user_email.clone()), role);

    spinner.succeed(&format!(
        "Logged in as {}  (role: {})",
        user_email,
        session::role()
    ));

    Ok(())
}
