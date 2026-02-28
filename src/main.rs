use std::{error::Error, io, time::Duration};

use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // add options for custom executable firefox (using local symlink to avoid hardcoding paths)
    let mut caps = DesiredCapabilities::firefox();
    let binary_path = std::env::current_dir()?.join("waterfox");
    caps.set_firefox_binary(binary_path.to_str().unwrap_or("waterfox"))?;

    let driver = WebDriver::new("http://localhost:4444", caps).await?;

    // Optional: Maximize window to ensure desktop view
    // This is important for the below code, bcz the page is responsive
    driver.maximize_window().await?;

    driver
        .goto("https://www.irctc.co.in/nget/train-search")
        .await?;

    // Wait for resize to complete and dialog to appear
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Handle the Alert dialog and click OK button
    // The OK button has type="submit" and class "btn btn-primary"
    match driver.find(By::Css("div[role='dialog']")).await {
        Ok(dialog) => {
            println!("Dialog found, looking for OK button...");

            // Find the OK button by its class and type
            let ok_button = dialog
                .find(By::Css("button.btn.btn-primary[type='submit']"))
                .await?;

            ok_button.click().await?;
            println!("OK button clicked successfully");
        }
        Err(e) => {
            println!("No dialog found or error: {}", e);
        }
    }

    // Try to find and click the izooto close button if it exists
    // CLOSE IZOOTO NOTIFICATION POPUP - Click "Later"
    println!("Checking for izooto notification popup...");
    match driver.find(By::Id("izooto-optin")).await {
        Ok(_) => {
            println!("Izooto popup found, clicking 'Later'...");
            let later_button = driver.find(By::Id("iz-optin-wp-btn1Txt")).await?;
            later_button.click().await?;
            tokio::time::sleep(Duration::from_millis(500)).await;
            println!("Popup dismissed");
        }
        Err(_) => println!("No izooto popup"),
    }

    // NOW CLICK LOGIN
    // STEP 1: Click Login button

    println!("Clicking LOGIN...");
    let login_button = driver
        .query(By::XPath("//a[contains(normalize-space(),'LOGIN')]"))
        .or(By::Css(".search_btn.loginText"))
        .first()
        .await?;

    // Use JavaScript click if normal click fails
    match login_button.click().await {
        Ok(_) => println!("Login clicked normally"),
        Err(_) => {
            println!("Normal click failed, using JavaScript click...");
            driver
                .execute("arguments[0].click();", vec![login_button.to_json()?])
                .await?;
        }
    }

    tokio::time::sleep(Duration::from_secs(3)).await;

    // HANDLE LOGIN MODAL - Find the modal dialog
    println!("Looking for login modal...");

    // The login modal should be visible now - look for input fields
    // Try multiple strategies to find the username field

    // Strategy 1: Wait for modal to be visible and find by placeholder
    let username_input = async {
        if let Ok(el) = driver
            .query(By::Css("input[formcontrolname='userid']"))
            .or(By::Css("input[placeholder='User Name']"))
            .wait(Duration::from_secs(5), Duration::from_millis(500))
            .first()
            .await
        {
            return Ok::<_, thirtyfour::error::WebDriverError>(el);
        }
        if let Ok(el) = driver.find(By::Css("p-dialog input[type='text']")).await {
            return Ok(el);
        }

        // Strategy 4: JavaScript injection - find by examining all inputs
        let r = driver
            .execute(
                "
                var inputs = document.querySelectorAll('input');
                for(var i=0; i<inputs.length; i++) {
                    if(inputs[i].placeholder && inputs[i].placeholder.includes('User')) {
                        return inputs[i];
                    }
                }
                return null;
            ",
                vec![],
            )
            .await?;
        match r.element() {
            Ok(el) => Ok(el),
            Err(_) => Err(thirtyfour::error::WebDriverError::IoError(
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Username input not found across all strategies",
                ),
            )),
        }
    }
    .await?;

    // Fill credentials...
    let username = std::env::var("USERNAME").expect("USERNAME must be set in .env");
    let password = std::env::var("PASSWORD").expect("PASSWORD must be set in .env");

    username_input.send_keys(&username).await?;

    let password_input = driver
        .query(By::Css("input[formcontrolname='password']"))
        .or(By::Css("input[placeholder='Password']"))
        .first()
        .await?;
    password_input.send_keys(&password).await?;

    println!("Credentials entered!");

    // Click Sign In button with comprehensive error handling
    println!("Attempting to click Sign In button...");
    match async {
        // Try multiple selectors
        let button = driver
            .query(By::XPath(
                "//button[contains(normalize-space(), 'SIGN IN')]",
            ))
            .or(By::Css("button[label='SIGN IN']"))
            .or(By::Css("button.login-button"))
            .or(By::Css("button.btn-primary[type='submit']"))
            .wait(Duration::from_secs(3), Duration::from_millis(500))
            .first()
            .await?;

        button.click().await?;
        Ok::<(), thirtyfour::error::WebDriverError>(())
    }
    .await
    {
        Ok(_) => println!("Sign In clicked successfully"),
        Err(e) => {
            eprintln!("Sign In button error: {}", e);
        }
    }

    // Prevent program from exiting
    println!("Browser is open.....");
    println!("Press Enter to close browser...");
    io::stdin().read_line(&mut String::new())?;

    // Now cleanup properly
    driver.quit().await?;
    Ok(())
}
