use postgrest::Postgrest;
use serde::Deserialize;
use solana_geyser_plugin_interface::geyser_plugin_interface::GeyserPluginError;
use solana_geyser_plugin_interface::geyser_plugin_interface::{
    GeyserPlugin, ReplicaAccountInfoVersions, Result as PluginResult,
};
use std::{
    error::Error,
    fmt::{self, Debug},
    fs::OpenOptions,
    io::Read,
};
use tokio::runtime::Runtime;

#[derive()]
pub struct SupabasePlugin {
    postgres_client: Option<Postgrest>,
    configuration: Option<Configuration>,
    programs: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub supabase_url: String,
    pub supabase_key: String,
    pub programs: Option<Vec<String>>,
}

impl Configuration {
    pub fn load(config_path: &str) -> Result<Self, Box<dyn Error>> {
        let mut file = OpenOptions::new().read(true).open(config_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        Ok(serde_json::from_str::<Configuration>(&contents)?)
    }
}

impl Default for SupabasePlugin {
    fn default() -> Self {
        SupabasePlugin {
            postgres_client: None,
            configuration: None,
            programs: Vec::new(),
        }
    }
}

impl GeyserPlugin for SupabasePlugin {
    fn name(&self) -> &'static str {
        "supabase-geyser"
    }

    fn on_load(&mut self, config_file: &str) -> PluginResult<()> {
        solana_logger::setup_with_default("info");
        println!("config file: {}", config_file);
        let config = match Configuration::load(config_file) {
            Ok(c) => c,
            Err(_e) => {
                return Err(GeyserPluginError::ConfigFileReadError {
                    msg: String::from("Error opening, or reading config file"),
                });
            }
        };

        self.postgres_client = Some(
            Postgrest::new(&config.supabase_url).insert_header("apikey", &config.supabase_key),
        );

        match config.programs.as_ref() {
            Some(accounts) => {
                accounts.iter().for_each(|account| {
                    let mut acc_bytes = [0u8; 32];
                    acc_bytes.copy_from_slice(&bs58::decode(account).into_vec().unwrap()[0..32]);
                    self.programs.push(acc_bytes);
                });
            }
            None => (),
        }

        self.configuration = Some(config);
        Ok(())
    }

    fn on_unload(&mut self) {}

    fn update_account(
        &mut self,
        account: ReplicaAccountInfoVersions,
        _slot: u64,
        _is_startup: bool,
    ) -> PluginResult<()> {
        let account_info = match account {
            ReplicaAccountInfoVersions::V0_0_1(_) => {
                return Err(GeyserPluginError::AccountsUpdateError {
                    msg: "ReplicaAccountInfoVersions::V0_0_1 it not supported".to_string(),
                });
            }
            ReplicaAccountInfoVersions::V0_0_2(account_info) => account_info,
        };

        println!(
            "account logged with publickey of: {:#?} and owner of {:#?} bytes",
            bs58::encode(account_info.pubkey).into_string(),
            account_info.owner.len()
        );
        self.programs.iter().for_each(|program| {
            // print hello if the account_info.owner is less than 32 bytes

            if program == account_info.owner {
                let account_pubkey = bs58::encode(account_info.pubkey).into_string();

                let rt = Runtime::new().unwrap();
                let result = rt.block_on(
                    self.postgres_client
                        .as_mut()
                        .unwrap()
                        .from("accounts")
                        .upsert(
                            serde_json::to_string(
                                &serde_json::json!([{ "account": account_pubkey, "owner": "yay" }]),
                            )
                            .unwrap(),
                        )
                        .execute(),
                );
                println!("result: {:#?}", result);
            } else {
            }
        });

        Ok(())
    }

    fn notify_end_of_startup(&mut self) -> PluginResult<()> {
        Ok(())
    }

    fn account_data_notifications_enabled(&self) -> bool {
        true
    }

    fn transaction_notifications_enabled(&self) -> bool {
        false
    }
}

impl Debug for SupabasePlugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupabasePlugin")
            .field("postgres_client", &self.postgres_client.is_some()) // Display whether the client exists or not
            .finish()
    }
}
