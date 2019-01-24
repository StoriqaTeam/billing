use stq_types::{Alpha3, ProxyCompanyBillingInfoId, SwiftId};

use schema::proxy_companies_billing_info;

#[derive(Clone, Serialize, Queryable, Insertable, Debug)]
#[table_name = "proxy_companies_billing_info"]
pub struct ProxyCompanyBillingInfo {
    pub id: ProxyCompanyBillingInfoId,
    pub country: Alpha3,
    pub swift_bic: SwiftId,
    pub bank_name: String,
    pub full_name: String,
    pub iban: String,
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset, Debug, Clone)]
#[table_name = "proxy_companies_billing_info"]
pub struct UpdateProxyCompanyBillingInfo {
    pub country: Option<Alpha3>,
    pub swift_bic: Option<SwiftId>,
    pub bank_name: Option<String>,
    pub full_name: Option<String>,
    pub iban: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "proxy_companies_billing_info"]
pub struct NewProxyCompanyBillingInfo {
    pub country: Alpha3,
    pub swift_bic: SwiftId,
    pub bank_name: String,
    pub full_name: String,
    pub iban: String,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct ProxyCompanyBillingInfoSearch {
    pub id: Option<ProxyCompanyBillingInfoId>,
    pub country: Option<Alpha3>,
}

impl ProxyCompanyBillingInfoSearch {
    pub fn by_country(country: Alpha3) -> ProxyCompanyBillingInfoSearch {
        ProxyCompanyBillingInfoSearch {
            country: Some(country),
            ..Default::default()
        }
    }
}
