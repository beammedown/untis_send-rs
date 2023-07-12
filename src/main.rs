use tokio;
use reqwest;
use serde_json::{self, json};
use chrono::{Duration, Local};
use std::env;
use dotenv::dotenv;
use log::LevelFilter;
use log::{debug, error, info, trace, warn, SetLoggerError};
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};

#[derive(Debug)]
pub enum Errnos {
    Reqwest(reqwest::Error),
    Serde(serde_json::Error),
    Env(env::VarError),
    Dotenv(dotenv::Error),
    Log(log::SetLoggerError),
    Log4rs(SetLoggerError),
    Box(Box<dyn std::error::Error>),
    Chrono(chrono::ParseError),
    Io(std::io::Error),
    Other(String),
}
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
        if response_json["result"].as_bool().unwrap() {
            info!("Successfully logged out.");
        } else {
            warn!("Logout failed.");
        }
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

fn get_message() -> String {
    let stundenplan = serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string("timetable.json").unwrap()).unwrap();
    let subjects = serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string("subjects.json").unwrap()).unwrap();
    let teachers = serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string("teachers.json").unwrap()).unwrap();

    let loctime = Local::now().naive_local();
    let lochour = loctime.format("%H").to_string().parse::<i32>().unwrap();
    let locday = loctime.format("%u").to_string().parse::<i32>().unwrap();
    
    let mut message = String::new();

    if locday < 6 && lochour == 7  {
        message = String::from("Heute entfällt:\n");
    } else if (locday < 6  && lochour <= 20) || (locday == 7  && lochour == 20) {
        message = String::from("Morgen entfällt:\n");
    } else {
        return String::new()
    }

    let stunden = json!({
        "750": 1,
        "840": 2,
        "940": 3,
        "1030": 4,
        "1130": 5,
        "1220": 6,
        "1335": 7,
        "1415": 8,
        "1505": 9,
        "1545": 10,
        "1625": 11,
        "1705": 12,
    });

    let stdpl = stundenplan["result"].as_array().unwrap();
    for i in stdpl {
        if i["kl"][0]["id"] == 661 && i["code"] == "cancelled" {

            let kuerzel = &i["su"][0]["id"];
            let hour = &stunden[&i["startTime"].to_string().parse::<String>().unwrap()];

            for j in subjects["result"].as_array().unwrap() {
                if &j["id"] == kuerzel {
                    let lul =  &teachers[&j["name"].to_string().parse::<String>().unwrap().strip_prefix("\"").unwrap().strip_suffix("\"").unwrap()];

                    message.push_str(&format!("{} in der {}. Stunde bei {}\n", &j["name"], hour, lul));
                    break;
                }
            }
        }
    }
    message
}

async fn send_message(chat_id: &str, message: &str, bottoken: &str) {
    let client = reqwest::Client::new();
    let params = json!({
        "chat_id": chat_id,
        "text": message,
    });
    let response = client.post(format!("https://api.telegram.org/bot{}/sendMessage", bottoken))
        .json(&params)
        .send()
        .await
        .unwrap();
    if response.status() != 200 {
        println!("Error: {}", response.status());
    }
}

fn config_log() -> Result<(), Box<dyn std::error::Error>> {
    let level = log::LevelFilter::Info;
    let file_path = "untis_send.log";

    // Build a stderr logger.
    let stderr = ConsoleAppender::builder().target(Target::Stderr).build();

    // Logging to log file.
    let logfile = FileAppender::builder()
        // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
        .encoder(Box::new(PatternEncoder::new("{d} - {h({l})} - {m}\n")))
        .build(file_path)
        .unwrap();

    // Log Trace level output to file where trace is the default level
    // and the programmatically specified level to stderr.
    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(level)))
                .build("stderr", Box::new(stderr)),
        )
        .build(
            Root::builder()
                .appender("logfile")
                .appender("stderr")
                .build(LevelFilter::Trace),
        )
        .unwrap();

    // Use this to change log levels at runtime.
    // This means you can change the default log level to trace
    // if you are trying to debug an issue and need more logs on then turn it off
    // once you are done.
    let _handle = log4rs::init_config(config)?;


    info!("Starting up.");
    Ok(())

}

#[tokio::main]
async fn main() -> Result<(), Errnos> {
    config_log().unwrap();

    info!("Logging configured.");

    dotenv().ok();
    let tel_chat_id = env::var("TELEGRAM_CHAT_ID").expect("TELEGRAM_CHAT_ID not found.");
    let tel_bottoken = env::var("TELEGRAM_BOTTOKEN").expect("TELEGRAM_BOTTOKEN not found.");
    info!("Telegram Env Vars found.");

    let mut untis = Untis::new(
        String::from(env::var("UNTIS_URL").expect("UNTIS_URL not found.")),
        String::from(env::var("UNTIS_SCHOOL").expect("UNTIS_SCHOOL not found.")),
        String::from(env::var("UNTIS_USERNAME").expect("UNTIS_USERNAME not found.")),
        String::from(env::var("UNTIS_PASSWORD").expect("UNTIS_PASSWORD not found.")),
        String::from(env::var("UNTIS_CLASS_ID").expect("UNTIS_USERAGENT not found.")),
    );
    info!("Untis configured.");

    untis.authenticate().await.unwrap();
    info!("Untis authenticated.");

    let subjects = untis.get_subjects().await.unwrap();
    info!("Subjects fetched.");

    let startdate = Local::now().naive_local().date();
    let enddate = startdate + Duration::days(1);
    info!("Startdate: {}, Enddate: {}", startdate, enddate);

    let timetable = untis.get_timetable(&startdate.format("%Y%m%d").to_string(), &enddate.format("%Y%m%d").to_string()).await.unwrap();
    info!("Timetable fetched.");

    let _ = std::fs::write("subjects.json", serde_json::to_string_pretty(&subjects).unwrap());
    let _ = std::fs::write("timetable.json", serde_json::to_string_pretty(&timetable).unwrap());
    info!("Files written.");

    untis.logout().await.unwrap();
    info!("Untis logged out.");

    let message = get_message();
    info!("Message created.");

    send_message(&tel_chat_id, &message, &tel_bottoken).await;
    info!("Message sent.");

    info!("Shutting down.");
    Ok(())

}
