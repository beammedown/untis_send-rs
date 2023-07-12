use tokio;
use reqwest;
use serde_json::{self, json};
use chrono::{self, Duration};
use std::process::exit;


#[derive(Clone, Debug)]
pub struct Untis {
    pub username: String,
    pub password: String,
    pub class_id: String,
    pub base_url: String,
    pub client: reqwest::Client,
    pub cookie: Option<String>,
}

impl Untis {
    pub fn new(base: String, school: String, username: String, password: String, class_id: String) -> Untis {
        let base_url = format!("https://{}.webuntis.com/WebUntis/jsonrpc.do?school={}", base, school);
        let client = reqwest::Client::new();
        Untis {
            username,
            password,
            class_id,
            base_url,
            client,
            cookie: None,
        }
    }

    pub async fn authenticate(&mut self) -> Result<(), reqwest::Error> {
        let params = json!({
            "user": self.username,
            "password": self.password,
            "client": "CLIENT",
        });
        let response = self.client.post(&self.base_url)
            .json(&json!({
                "id": "ID",
                "method": "authenticate",
                "params": params,
                "jsonrpc": "2.0",
            }))
            .send()
            .await?;

        let response_json: serde_json::Value = response.json().await?;
        self.cookie = Some(response_json["result"]["sessionId"].as_str().unwrap().to_string());
        
        println!("{:#?}", response_json);

        println!("Unti: {:#?}", self);
        Ok(())
    }
    
    pub async fn logout(&self) -> Result<(), reqwest::Error> {
        let response = self.client.post(&self.base_url)
            .json(&json!({
                "id": "ID",
                "method": "logout",
                "params": {},
                "jsonrpc": "2.0",
            }))
            .header("Cookie", format!("JSESSIONID={}", &self.cookie.clone().expect("No cookie found.")))
            .send()
            .await?;

        let response_json: serde_json::Value = response.json().await?;

        Ok(())
    }

    pub async fn get_timetable(&self, startdate: &String, enddate: &String) -> Result<serde_json::Value, reqwest::Error> {
        let params = json!({
            "id": self.class_id,
            "type": 1,
            "startDate": startdate,
            "endDate": enddate,
        });

        let response = self.client.post(&self.base_url)
            .json(&json!({
                "id": "ID",
                "method": "getTimetable",
                "params": params,
                "jsonrpc": "2.0",
            }))
            .header("Cookie", format!("JSESSIONID={}", &self.cookie.clone().expect("No cookie found.")))
            .send()
            .await?;

        let response_json: serde_json::Value = response.json().await?;

        Ok(response_json)
    }

    pub async fn get_subjects(&self) -> Result<serde_json::Value, reqwest::Error> {
        let response = self.client.post(&self.base_url)
            .json(&json!({
                "id": "ID",
                "method": "getSubjects",
                "params": {},
                "jsonrpc": "2.0",
            }))
            .header("Cookie", format!("JSESSIONID={}", &self.cookie.clone().expect("No cookie found.")))
            .send()
            .await?;
        let response_json: serde_json::Value = response.json().await?;

        Ok(response_json)
    }
}

fn get_sleeptime() -> u32 {
    // The Function should return the time until 07 am or 8 pm in 24 hour
    let current_time = chrono::Local::now();
    if current_time.date_naive().format("%A").to_string() == "Friday" && current_time.time().format("%H").to_string().parse::<i32>().unwrap() > 7 {
        return 60*60*24*2;
    } else if (current_time.date_naive().format("%A").to_string() == "Sunday") && (current_time.time().format("%H").to_string().parse::<i32>().unwrap() < 20) {
        return 60*60*13;
    } else if (current_time.date_naive().format("%A").to_string() == "Sunday") && (current_time.time().format("%H").to_string().parse::<i32>().unwrap() >= 20) {
        return 60*60*24*6;
    } else if current_time.time().format("%H").to_string().parse::<i32>().unwrap() < 7 {
        return 60*60*(7-current_time.time().format("%H").to_string().parse::<i32>().unwrap()) as u32;
    } else if current_time.time().format("%H").to_string().parse::<i32>().unwrap() >= 20 {
        return 60*60*(31-current_time.time().format("%H").to_string().parse::<i32>().unwrap()) as u32;
    } else {
        return 60*60*13;
    }
}
#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let mut untis = Untis::new(
        String::from("mese"),
        String::from("JL-Schule+Darmstadt"),
        String::from("LiO-Lernende"),
        String::from("Schueler.2021"),
        String::from("661")
    );
    untis.authenticate().await?;
    let subjects = untis.get_subjects().await?;


    let startdate = chrono::Local::now().naive_local().date();
    let enddate = startdate + Duration::days(1);
    
    let timetable = untis.get_timetable(&startdate.format("%Y%m%d").to_string(), &enddate.format("%Y%m%d").to_string()).await?;

    let _ = std::fs::write("subjects.json", serde_json::to_string_pretty(&subjects).unwrap());
    let _ = std::fs::write("timetable.json", serde_json::to_string_pretty(&timetable).unwrap());

    untis.logout().await?;

    loop {
        let sleeptime = get_sleeptime();
        std::thread::sleep(std::time::Duration::from_secs(sleeptime.into()));
        send_message();
    }
    send_error_message();
}
