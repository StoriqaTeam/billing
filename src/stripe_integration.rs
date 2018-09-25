use libstripe::*;

use stq_types::stripe::SourceId;

#[derive(Debug, Deserialize, Clone)]
pub struct StripeConfig {
    pub api_key: String,
}

pub fn create_charge(config: &StripeConfig, source_id: SourceId ) { // TODO impl amount and currency
    
    let api_key = config.api_key.clone();
    let client = Client::new(&api_key);
    
    let mut charge_param = ChargeParams::default();
    charge_param.amount = Some(2000);
    charge_param.currency = Some(Currency::USD);
    charge_param.source = Some(PaymentSourceParam::Token(&source_id.0));

    let charge = match Charge::create(&client, charge_param) {
        Ok(c) => c,
        Err(e) => panic!("{}", e)
    };

    println!("{:?}", charge);
}