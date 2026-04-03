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

    // After signin, input the two station fields from environment variables
    let source_station =
        std::env::var("SOURCE_STATION").expect("SOURCE_STATION must be set in .env");
    let dest_station = std::env::var("DEST_STATION").expect("DEST_STATION must be set in .env");
    let journey_date = std::env::var("JOURNEY_DATE").expect("JOURNEY_DATE must be set in .env");
    let journey_class = std::env::var("JOURNEY_CLASS").expect("JOURNEY_CLASS must be set in .env");
    let journey_quota = std::env::var("JOURNEY_QUOTA").unwrap_or_else(|_| "GENERAL".to_string());

    println!("Waiting for source station input field...");
    let input_field1 = driver
        .query(By::Css("input[aria-controls='pr_id_1_list']"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    println!("Entering source station: {}", source_station);
    input_field1.send_keys(&source_station).await?;

    println!("Waiting for destination station input field...");
    let input_field2 = driver
        .query(By::Css("input[aria-controls='pr_id_2_list']"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;
    println!("Entering destination station: {}", dest_station);
    input_field2.send_keys(&dest_station).await?;

    println!("Waiting for journey date input field...");
    // Finding the date field in IRCTC (often a p-calendar input with class ui-calendar)
    let date_field = driver
        .query(By::Css(
            "p-calendar input[type='text'], input.ui-inputtext.ui-widget",
        ))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;

    // We clear the date field by sending Ctrl+a / Command+a followed by Delete, because simple .clear() sometimes doesn't work for frontend framework calendars
    date_field.send_keys(thirtyfour::Key::Control + "a").await?;
    date_field.send_keys(thirtyfour::Key::Backspace).await?;

    println!("Entering journey date: {}", journey_date);
    date_field.send_keys(&journey_date).await?;
    // Send an Enter or Tab key to close the calendar popup
    date_field.send_keys(thirtyfour::Key::Tab).await?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    println!("Selecting journey class: {}", journey_class);
    // Find and click the class dropdown
    let class_dropdown = driver
        .query(By::Id("journeyClass"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;

    // Attempt to click the dropdown to open it
    class_dropdown.click().await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Find the option by aria-label
    let class_option_css = format!("li[aria-label='{}']", journey_class);
    let class_option = driver
        .query(By::Css(&class_option_css))
        .wait(Duration::from_secs(5), Duration::from_millis(500))
        .first()
        .await?;

    class_option.click().await?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    println!("Selecting journey quota: {}", journey_quota);
    // Find and click the quota dropdown
    let quota_dropdown = driver
        .query(By::Id("journeyQuota"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;

    // Attempt to click the dropdown to open it
    quota_dropdown.click().await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Find the option by aria-label
    let quota_option_css = format!("li[aria-label='{}']", journey_quota);
    let quota_option = driver
        .query(By::Css(&quota_option_css))
        .wait(Duration::from_secs(5), Duration::from_millis(500))
        .first()
        .await?;

    quota_option.click().await?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Prevent program from exiting
    println!("Browser is open.....");
    println!("Press Enter to close browser...");
    io::stdin().read_line(&mut String::new())?;

    // Now cleanup properly
    driver.quit().await?;
    Ok(())
}
