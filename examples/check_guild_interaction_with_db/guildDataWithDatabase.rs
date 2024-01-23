use sf_api::sso::SFAccount;
use mysql_async::*;
use mysql_async::prelude::*;
use sf_api::gamestate::GameState;
use mysql_async::{Pool, Opts, Result};

#[tokio::main]
async fn main() {
    match load_sf().await {
        Ok(_) => println!("Operation completed successfully."),
        Err(e) => eprintln!("An error occurred: {}", e),
    }
}

fn create_database_url<'a>(username: &str, password: &str, host: &str, port: u16, database_name: &str) -> String {
    format!("mysql://{}:{}@{}:{}/{}", username, password, host, port, database_name)
}

async fn load_sf() -> Result<()> {
    let database_url = create_database_url("", "", "", 3306, "");
    let pool = establish_connection(&database_url).await?;
    let account = SFAccount::login(
        "username".to_string(),
        "password".to_string()
    ).await.unwrap();

    for mut session in account.characters().await.unwrap().into_iter().flatten()
    {
        if session.server_url().to_string() == "https://s7.sfgame.eu/" { // Specific server
            let response = session.login().await.unwrap();
            let _game_state = GameState::new(response).unwrap();

            let guild = _game_state.unlocks.guild.unwrap();
            let is_def = guild.defense_date;
            let is_attack = guild.attack_date;

            let mut conn = pool.get_conn().await?;
            if let Some(def_date) = is_def {
                println!("{}", def_date.to_string());
                let message = format!("[[SF]](https://vlcizcech.tech/) Varování! Nepřátelský útok je na obzoru, nezbytné okamžitě se připravit na obranu! Čas útoku je {}", def_date.to_string());
                let query = format!("SELECT EXISTS(SELECT 1 FROM discord_queue WHERE message = '{}')", &message);
                let exists: bool = conn.query_first(query)
                    .await?
                    .unwrap_or(false);

                if !exists {
                    conn.exec_drop(
                        r"INSERT INTO discord_queue (message, channel, status) VALUES (:message, :channel, 'pending')",
                        params! {
                            "message" => &message,
                            "channel" => "1189908627373428788",
                        }
                    ).await?;
                }
            } else {
                println!("No defense date available");
            }

            if let Some(attack_date) = is_attack {
                println!("{}", attack_date.to_string());
                let message = format!("[[SF]](https://vlcizcech.tech/) Upozornění! Zahájili jsme útok, je nezbytné se okamžitě připravit na další fázi naší ofenzívy! Čas útoku je {}", attack_date.to_string());
                let query = format!("SELECT EXISTS(SELECT 1 FROM discord_queue WHERE message = '{}')", &message);
                let exists: bool = conn.query_first(query)
                    .await?
                    .unwrap_or(false);

                if !exists {
                    conn.exec_drop(
                        r"INSERT INTO discord_queue (message, channel, status) VALUES (:message, :channel, 'pending')",
                        params! {
                            "message" => &message,
                            "channel" => "1182042940525252658",
                        }
                    ).await?;
                }
            } else {
                println!("No attack date available");
            }
        }
    }

    Ok(())
}