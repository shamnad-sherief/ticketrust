use serde::Deserialize;
use std::{error::Error, io, time::Duration};

use thirtyfour::prelude::*;

#[derive(Deserialize, Debug)]
struct Passenger {
    name: String,
    age: String,
    gender: String,
    pref: Option<String>,
}

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

    // Parse train numbers handling comma separated string
    let train_numbers_env = std::env::var("TRAIN_NUMBERS").unwrap_or_default();
    let target_trains: Vec<String> = train_numbers_env
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let passengers_env = std::env::var("PASSENGERS").unwrap_or_else(|_| "[]".to_string());
    let passengers: Vec<Passenger> = serde_json::from_str(&passengers_env).expect("PASSENGERS must be a valid JSON array");

    if passengers.is_empty() {
        panic!("No passengers found in PASSENGERS environment variable");
    }

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
    // Finding the date field in IRCTC precisely by its ID
    let date_field = driver
        .query(By::Css("#jDate input"))
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

    println!("Clicking Search Trains button...");
    let search_button = driver
        .query(By::Css("button.train_Search"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first()
        .await?;

    // Sometimes a direct click doesn't work if elements overlay it, so we fallback to JS click if needed
    match search_button.click().await {
        Ok(_) => {}
        Err(_) => {
            driver
                .execute("arguments[0].click();", vec![search_button.to_json()?])
                .await?;
        }
    }
    println!("Search initiated!");

    // Give time for search results to load
    tokio::time::sleep(Duration::from_secs(3)).await;

    println!("Waiting for train result cards to load...");
    // Wait until at least one train card is visible
    let _ = driver
        .query(By::Css(
            "div.form-group.no-pad.col-xs-12.bull-back.border-all",
        ))
        .wait(Duration::from_secs(20), Duration::from_millis(500))
        .first()
        .await?;

    let train_cards = driver
        .query(By::Css(
            "div.form-group.no-pad.col-xs-12.bull-back.border-all",
        ))
        .all_from_selector()
        .await?;

    println!("Found {} train cards", train_cards.len());

    let mut selected_card: Option<WebElement> = None;

    if target_trains.is_empty() {
        println!("No TRAIN_NUMBERS specified. Proceeding with the first available train.");
        selected_card = train_cards.into_iter().next();
    } else {
        println!("Looking for trains matching: {:?}", target_trains);
        for card in train_cards {
            if let Ok(heading) = card.find(By::Css(".train-heading strong")).await {
                if let Ok(text) = heading.text().await {
                    // Check if this train matches any in our list
                    if target_trains.iter().any(|t| text.contains(t)) {
                        println!("Found target train: {}", text);
                        selected_card = Some(card);
                        break; // Stop at the first preferred matched train
                    }
                }
            }
        }
    }

    if let Some(card) = selected_card {
        println!(
            "Looking for class match: '{}' in the selected train",
            journey_class
        );

        // Find the clickable class blocks inside the train card
        if let Ok(class_blocks) = card.query(By::Css(".pre-avl")).all_from_selector().await {
            let mut clicked = false;
            for block in class_blocks {
                if let Ok(block_text) = block.text().await {
                    if block_text.contains(&journey_class) {
                        println!("Found matching class block: {}", block_text);
                        // Click the class box to trigger availability check
                        // We use javascript click here because Angular component overlaps can be tricky
                        let r = driver
                            .execute("arguments[0].click();", vec![block.to_json()?])
                            .await;
                        if r.is_err() {
                            let _ = block.click().await;
                        }
                        clicked = true;
                        break;
                    }
                }
            }

            if !clicked {
                println!(
                    "Could not find the class '{}' block or it was unavailable.",
                    journey_class
                );
            } else {
                // If we successfully clicked the class block, now wait for the availability dates to expand and click the Book Now button
                println!("Waiting for availability data to expand...");
                tokio::time::sleep(Duration::from_secs(3)).await;

                // Attempt to click the first availability date box
                println!("Clicking on the availability date box...");
                if let Ok(date_block) = card
                    .query(By::Css("table td div.pre-avl, div.ui-table td div"))
                    .wait(Duration::from_secs(5), Duration::from_millis(500))
                    .first()
                    .await
                {
                    let r = driver
                        .execute("arguments[0].click();", vec![date_block.to_json()?])
                        .await;
                    if r.is_err() {
                        let _ = date_block.click().await;
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                } else {
                    println!(
                        "Could not find specific date block, proceeding to click Book Now directly if possible..."
                    );
                }

                println!("Clicking 'Book Now' button...");
                if let Ok(book_now_btn) = card
                    .query(By::Css(
                        "button.btnDefault.train_Search, button.disable-book",
                    ))
                    .wait(Duration::from_secs(5), Duration::from_millis(500))
                    .first()
                    .await
                {
                    let r = driver
                        .execute("arguments[0].click();", vec![book_now_btn.to_json()?])
                        .await;
                    if r.is_err() {
                        let _ = book_now_btn.click().await;
                    }
                    println!("Successfully clicked Book Now!");

                    // Proceed to enter passenger details
                    println!("Waiting for Passenger Details page...");
                    tokio::time::sleep(Duration::from_secs(5)).await;

                    for (i, passenger) in passengers.iter().enumerate() {
                        println!("Processing passenger {}: {}", i + 1, passenger.name);

                        if i > 0 {
                            println!("Clicking '+ Add Passenger' for passenger {}...", i + 1);
                            let add_btn = driver
                                .query(By::XPath("//span[contains(normalize-space(), '+ Add Passenger')]"))
                                .wait(Duration::from_secs(5), Duration::from_millis(500))
                                .first()
                                .await?;
                            add_btn.click().await?;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }

                        // Passenger Name
                        let name_inputs = driver
                            .query(By::Css(
                                "input[placeholder='Name'], p-autocomplete#passengerName input",
                            ))
                            .all_from_selector()
                            .await?;

                        if let Some(name_input) = name_inputs.get(i) {
                            println!("Entering Passenger Name...");
                            name_input.send_keys(&passenger.name).await?;
                        } else {
                            println!("Could not find Passenger Name input for index {}.", i);
                        }

                        // Passenger Age
                        let age_inputs = driver
                            .query(By::Css(
                                "input[placeholder='Age'], input[formcontrolname='passengerAge']",
                            ))
                            .all_from_selector()
                            .await?;

                        if let Some(age_input) = age_inputs.get(i) {
                            println!("Entering Passenger Age...");
                            age_input.send_keys(&passenger.age).await?;
                        } else {
                            println!("Could not find Passenger Age input for index {}.", i);
                        }

                        // Passenger Gender
                        let gender_selects = driver
                            .query(By::Css(
                                "select[formcontrolname='passengerGender'], p-dropdown[formcontrolname='passengerGender']",
                            ))
                            .all_from_selector()
                            .await?;

                        if let Some(gender_select) = gender_selects.get(i) {
                            println!("Selecting Passenger Gender...");
                            let _ = gender_select.send_keys(&passenger.gender).await;
                            let _ = gender_select.send_keys(thirtyfour::Key::Enter).await;
                        } else {
                            println!("Could not find Passenger Gender select for index {}.", i);
                        }

                        // Passenger Berth Preference
                        if let Some(pref) = &passenger.pref {
                            if !pref.trim().is_empty() {
                                let pref_selects = driver
                                    .query(By::Css(
                                        "select[formcontrolname='passengerBerthChoice'], p-dropdown[formcontrolname='passengerBerthChoice']",
                                    ))
                                    .all_from_selector()
                                    .await?;

                                if let Some(pref_select) = pref_selects.get(i) {
                                    println!("Selecting Passenger Preference: {}", pref);
                                    let _ = pref_select.send_keys(pref).await;
                                } else {
                                    println!("Could not find Passenger Berth Preference for index {}.", i);
                                }
                            } else {
                                println!("Skipping preference for passenger {} (empty)", i + 1);
                            }
                        } else {
                            println!("Skipping preference for passenger {} (None)", i + 1);
                        }

                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }

                    // IRCTC Co-branded Card Benefits: Select "Skip"
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    println!("Step: Selecting IRCTC Co-branded Card Benefits: Skip...");
                    // Try to find the radio button box within the Skip component (id lo-3)
                    let skip_radio_xpath = "//p-radiobutton[@id='lo-3']//div[contains(@class, 'ui-radiobutton-box')]";
                    if let Ok(skip_radio) = driver.query(By::XPath(skip_radio_xpath)).first().await {
                        println!("Found 'Skip' radio box, scrolling and clicking...");
                        let _ = driver.execute("arguments[0].scrollIntoView(true);", vec![skip_radio.to_json()?]).await;
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        let _ = driver.execute("arguments[0].click();", vec![skip_radio.to_json()?]).await;
                        println!("Clicked 'Skip' radio button.");
                    } else {
                        // Fallback to label search
                        println!("Warning: Could not find 'lo-3' radio box, trying label fallback...");
                        let skip_label_xpath = "//label[contains(normalize-space(), 'Skip')]";
                        if let Ok(skip_label) = driver.query(By::XPath(skip_label_xpath)).first().await {
                            let _ = driver.execute("arguments[0].click();", vec![skip_label.to_json()?]).await;
                            println!("Clicked 'Skip' label.");
                        } else {
                            println!("Error: Could not find any element for 'Skip' radio button.");
                        }
                    }

                    // Other Preferences: Consider for Auto Upgradation
                    println!("Step: Checking 'Consider for Auto Upgradation' checkbox...");
                    let auto_up_xpath = "//input[@id='autoUpgradation'] | //label[@for='autoUpgradation']";
                    if let Ok(auto_up) = driver.query(By::XPath(auto_up_xpath)).first().await {
                        println!("Found 'Auto Upgradation' option, scrolling and clicking...");
                        let _ = driver.execute("arguments[0].scrollIntoView(true);", vec![auto_up.to_json()?]).await;
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        let _ = driver.execute("arguments[0].click();", vec![auto_up.to_json()?]).await;
                        println!("Clicked 'Auto Upgradation' checkbox.");
                    } else {
                        println!("Warning: Could not find 'Consider for Auto Upgradation' checkbox.");
                    }

                    // Click Continue Button
                    println!("Looking for Continue button...");
                    if let Ok(continue_btn) = driver.query(By::XPath("//button[contains(translate(., 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz'), 'continue')]")).wait(Duration::from_secs(5), Duration::from_millis(500)).first().await {
                        println!("Clicking Continue...");
                        let r = driver.execute("arguments[0].click();", vec![continue_btn.to_json()?]).await;
                        if r.is_err() {
                            let _ = continue_btn.click().await;
                        }
                        println!("Successfully clicked Continue!");
                        
                        println!("\n=======================================================");
                        println!("CAPTCHA REQUIRED: Please solve it in the browser window");
                        println!("and manually click 'Continue' on the Review page.");
                        println!("Waiting to securely reach the Payment page...");
                        println!("=======================================================\n");
                        
                        // Poll for URL change indicating the payment page was reached
                        loop {
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            if let Ok(url) = driver.current_url().await {
                                if url.as_str().to_lowercase().contains("payment") {
                                    println!("✅ Payment Page detected!");
                                    break;
                                }
                            }
                        }
                        
                        // Further Payment logic goes here later!
                        println!("Ready to handle payment selection...");
                        
                    } else {
                        println!("Could not find the Continue button.");
                    }
                } else {
                    println!("Error: Could not find 'Book Now' button. Is availability open?");
                }
            }
        } else {
            println!("Could not find any class blocks inside the train card.");
        }
    } else {
        println!("Could not find any of the target trains in the search results.");
    }

    // Prevent program from exiting
    println!("Browser is open.....");
    println!("Press Enter to close browser...");
    io::stdin().read_line(&mut String::new())?;

    // Now cleanup properly
    driver.quit().await?;
    Ok(())
}
