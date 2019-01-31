use std::collections::HashMap;

use stq_static_resources::Currency as StqCurrency;
use stq_types::{CurrencyExchangeId, ExchangeRate};

use models::{currency::ConversionError as CurrencyConversionError, Currency};

pub type ExchangeRatesRequest = HashMap<StqCurrency, ExchangeRate>;
pub type ExchangeRates = HashMap<Currency, ExchangeRate>;

pub type CurrencyExchangeDataRequest = HashMap<StqCurrency, ExchangeRatesRequest>;
pub type CurrencyExchangeData = HashMap<Currency, ExchangeRates>;

#[derive(Clone, Debug, Deserialize)]
pub struct CurrencyExchangeInfoRequest {
    pub id: CurrencyExchangeId,
    pub data: CurrencyExchangeDataRequest,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CurrencyExchangeInfo {
    pub id: CurrencyExchangeId,
    pub data: CurrencyExchangeData,
}

impl CurrencyExchangeInfo {
    pub fn try_from_request(other: CurrencyExchangeInfoRequest) -> Result<Self, CurrencyConversionError> {
        let data = other
            .data
            .into_iter()
            .map(|(stq_currency, stq_rates)| {
                Currency::try_from_stq_currency(stq_currency)
                    .map_err(|_| CurrencyConversionError::UnsupportedCurrency(stq_currency.to_string()))
                    .and_then(|currency| try_exchange_rates_from_request(stq_rates).map(|rates| (currency, rates)))
            })
            .collect::<Result<CurrencyExchangeData, CurrencyConversionError>>()?;

        Ok(Self { id: other.id, data })
    }
}

pub fn try_exchange_rates_from_request(other: ExchangeRatesRequest) -> Result<ExchangeRates, CurrencyConversionError> {
    other
        .into_iter()
        .map(|(stq_rate_currency, stq_rate)| {
            Currency::try_from_stq_currency(stq_rate_currency)
                .map_err(|_| CurrencyConversionError::UnsupportedCurrency(stq_rate_currency.to_string()))
                .map(move |rate_currency| (rate_currency, stq_rate))
        })
        .collect::<Result<ExchangeRates, CurrencyConversionError>>()
}
