use stq_static_resources::Currency;
use stq_types::{Alpha3, ProxyCompanyBillingInfoId, SwiftId};

use schema::proxy_companies_billing_info;

#[derive(Clone, Serialize, Queryable, Insertable, Debug)]
#[table_name = "proxy_companies_billing_info"]
pub struct ProxyCompanyBillingInfo {
    pub id: ProxyCompanyBillingInfoId,
    pub country_alpha3: Alpha3,
    pub account: String,
    pub currency: Currency,
    pub name: String,
    pub bank: String,
    pub swift: SwiftId,
    pub bank_address: String,
    pub country: String,
    pub city: String,
    pub recipient_address: String,
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset, Debug, Clone, Default)]
#[table_name = "proxy_companies_billing_info"]
pub struct UpdateProxyCompanyBillingInfo {
    pub country_alpha3: Option<Alpha3>,
    pub account: Option<String>,
    pub currency: Option<Currency>,
    pub name: Option<String>,
    pub bank: Option<String>,
    pub swift: Option<SwiftId>,
    pub bank_address: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub recipient_address: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "proxy_companies_billing_info"]
pub struct NewProxyCompanyBillingInfo {
    pub country_alpha3: Alpha3,
    pub account: String,
    pub currency: Currency,
    pub name: String,
    pub bank: String,
    pub swift: SwiftId,
    pub bank_address: String,
    pub country: String,
    pub city: String,
    pub recipient_address: String,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct ProxyCompanyBillingInfoSearch {
    pub id: Option<ProxyCompanyBillingInfoId>,
    pub country_alpha3: Option<Alpha3>,
}

impl ProxyCompanyBillingInfoSearch {
    pub fn by_country_alpha3(country_alpha3: Alpha3) -> ProxyCompanyBillingInfoSearch {
        ProxyCompanyBillingInfoSearch {
            country_alpha3: Some(country_alpha3),
            ..Default::default()
        }
    }
}
