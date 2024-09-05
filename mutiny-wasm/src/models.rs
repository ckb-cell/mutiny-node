use ::nostr::ToBech32;
use bitcoin::secp256k1::{PublicKey, ThirtyTwoByteHash};
use bitcoin::OutPoint;
use gloo_utils::format::JsValueSerdeExt;
use hex_conservative::DisplayHex;
use lightning_invoice::{Bolt11Invoice, Bolt11InvoiceDescription};
use lnurl::lightning_address::LightningAddress;
use lnurl::lnurl::LnUrl;
use mutiny_core::event::HTLCStatus;
use mutiny_core::labels::Contact as MutinyContact;
use mutiny_core::nostr::nwc::SpendingConditions;
use mutiny_core::*;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::json;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[wasm_bindgen]
pub enum ActivityType {
    OnChain,
    Lightning,
    ChannelOpen,
    ChannelClose,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[wasm_bindgen]
pub struct ActivityItem {
    pub kind: ActivityType,
    id: String,
    pub amount_sats: Option<u64>,
    pub inbound: bool,
    pub(crate) labels: Vec<String>,
    pub(crate) contacts: Vec<TagItem>,
    pub last_updated: Option<u64>,
    privacy_level: String,
}

#[wasm_bindgen]
impl ActivityItem {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn labels(&self) -> Vec<String> {
        self.labels.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn contacts(&self) -> Vec<TagItem> {
        self.contacts.clone()
    }
}

impl From<mutiny_core::ActivityItem> for ActivityItem {
    fn from(a: mutiny_core::ActivityItem) -> Self {
        let kind = match a {
            mutiny_core::ActivityItem::OnChain(_) => {
                if a.is_channel_open() {
                    ActivityType::ChannelOpen
                } else {
                    ActivityType::OnChain
                }
            }
            mutiny_core::ActivityItem::Lightning(_) => ActivityType::Lightning,
            mutiny_core::ActivityItem::ChannelClosed(_) => ActivityType::ChannelClose,
        };

        let id = match a {
            mutiny_core::ActivityItem::OnChain(ref t) => t.internal_id.to_string(),
            mutiny_core::ActivityItem::Lightning(ref ln) => {
                ln.payment_hash.into_32().to_lower_hex_string()
            }
            mutiny_core::ActivityItem::ChannelClosed(ref c) => c
                .user_channel_id
                .map(|c| c.to_lower_hex_string())
                .unwrap_or_default(),
        };

        let (inbound, amount_sats) = match a {
            mutiny_core::ActivityItem::OnChain(ref t) => {
                let inbound = t.received > t.sent;
                let amount_sats = if inbound {
                    Some(t.received - t.sent)
                } else {
                    Some(t.sent - t.received)
                };
                (inbound, amount_sats)
            }
            mutiny_core::ActivityItem::Lightning(ref ln) => (ln.inbound, ln.amount_sats),
            mutiny_core::ActivityItem::ChannelClosed(_) => (false, None),
        };

        let privacy_level = match kind {
            ActivityType::OnChain => PrivacyLevel::NotAvailable,
            ActivityType::Lightning => {
                if let mutiny_core::ActivityItem::Lightning(ref ln) = a {
                    ln.privacy_level
                } else {
                    PrivacyLevel::NotAvailable
                }
            }
            ActivityType::ChannelOpen => PrivacyLevel::NotAvailable,
            ActivityType::ChannelClose => PrivacyLevel::NotAvailable,
        };

        ActivityItem {
            kind,
            id,
            amount_sats,
            inbound,
            labels: a.labels(),
            contacts: vec![],
            last_updated: a.last_updated(),
            privacy_level: privacy_level.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[wasm_bindgen]
pub struct MutinyInvoice {
    bolt11: Option<Bolt11Invoice>,
    description: Option<String>,
    payment_hash: String,
    preimage: Option<String>,
    payee_pubkey: Option<String>,
    pub amount_sats: Option<u64>,
    pub expire: u64,
    pub expired: bool,
    status: String,
    privacy_level: String,
    pub fees_paid: Option<u64>,
    pub inbound: bool,
    pub last_updated: u64,
    pub potential_hodl_invoice: bool,
    labels: Vec<String>,
}

#[wasm_bindgen]
impl MutinyInvoice {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }

    #[wasm_bindgen(getter)]
    pub fn bolt11(&self) -> Option<String> {
        self.bolt11.clone().map(|b| b.to_string())
    }

    #[wasm_bindgen(getter)]
    pub fn description(&self) -> Option<String> {
        self.description.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn payment_hash(&self) -> String {
        self.payment_hash.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn preimage(&self) -> Option<String> {
        self.preimage.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn payee_pubkey(&self) -> Option<String> {
        self.payee_pubkey.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn status(&self) -> String {
        self.status.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn privacy_level(&self) -> String {
        self.privacy_level.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn paid(&self) -> bool {
        self.status == HTLCStatus::Succeeded.to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn labels(&self) -> Vec<String> {
        self.labels.clone()
    }
}

impl From<mutiny_core::MutinyInvoice> for MutinyInvoice {
    fn from(m: mutiny_core::MutinyInvoice) -> Self {
        let potential_hodl_invoice = match m.bolt11 {
            Some(ref b) => utils::is_hodl_invoice(b),
            None => false,
        };
        let now = utils::now().as_secs();
        MutinyInvoice {
            bolt11: m.bolt11,
            description: m.description,
            payment_hash: m.payment_hash.into_32().to_lower_hex_string(),
            preimage: m.preimage,
            payee_pubkey: m.payee_pubkey.map(|p| p.serialize().to_lower_hex_string()),
            amount_sats: m.amount_sats,
            expire: m.expire,
            expired: m.expire < now,
            status: m.status.to_string(),
            privacy_level: m.privacy_level.to_string(),
            fees_paid: m.fees_paid,
            inbound: m.inbound,
            last_updated: m.last_updated,
            potential_hodl_invoice,
            labels: m.labels,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[wasm_bindgen]
pub struct MutinyPeer {
    pubkey: String,
    connection_string: Option<String>,
    alias: Option<String>,
    color: Option<String>,
    label: Option<String>,
    pub is_connected: bool,
}

#[wasm_bindgen]
impl MutinyPeer {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }

    #[wasm_bindgen(getter)]
    pub fn pubkey(&self) -> String {
        self.pubkey.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn connection_string(&self) -> Option<String> {
        self.connection_string.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn alias(&self) -> Option<String> {
        self.alias.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn color(&self) -> Option<String> {
        self.color.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn label(&self) -> Option<String> {
        self.label.clone()
    }
}

impl From<nodemanager::MutinyPeer> for MutinyPeer {
    fn from(m: nodemanager::MutinyPeer) -> Self {
        MutinyPeer {
            pubkey: m.pubkey.serialize().to_lower_hex_string(),
            connection_string: m.connection_string,
            alias: m.alias,
            color: m.color,
            label: m.label,
            is_connected: m.is_connected,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[wasm_bindgen]
pub struct MutinyChannel {
    user_chan_id: String,
    pub balance: u64,
    pub size: u64,
    pub reserve: u64,
    pub inbound: u64,
    outpoint: Option<String>,
    peer: String,
    pub confirmations_required: Option<u32>,
    pub confirmations: u32,
    pub is_outbound: bool,
    pub is_usable: bool,
    pub is_anchor: bool,
}

#[wasm_bindgen]
impl MutinyChannel {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }

    #[wasm_bindgen(getter)]
    pub fn user_chan_id(&self) -> String {
        self.user_chan_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn outpoint(&self) -> Option<String> {
        self.outpoint.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn peer(&self) -> String {
        self.peer.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn confirmed(&self) -> bool {
        match self.confirmations_required {
            Some(c) => self.confirmations >= c,
            None => false,
        }
    }
}

impl From<nodemanager::MutinyChannel> for MutinyChannel {
    fn from(m: nodemanager::MutinyChannel) -> Self {
        MutinyChannel {
            user_chan_id: m.user_chan_id,
            balance: m.balance,
            size: m.size,
            reserve: m.reserve,
            inbound: m.inbound,
            outpoint: m.outpoint.map(|o| o.to_string()),
            peer: m.peer.serialize().to_lower_hex_string(),
            confirmations_required: m.confirmations_required,
            confirmations: m.confirmations,
            is_outbound: m.is_outbound,
            is_usable: m.is_usable,
            is_anchor: m.is_anchor,
        }
    }
}

impl From<MutinyChannel> for nodemanager::MutinyChannel {
    fn from(m: MutinyChannel) -> Self {
        nodemanager::MutinyChannel {
            user_chan_id: m.user_chan_id,
            balance: m.balance,
            size: m.size,
            reserve: m.reserve,
            inbound: m.inbound,
            outpoint: m
                .outpoint
                .map(|o| OutPoint::from_str(&o).expect("Invalid outpoint")),
            peer: PublicKey::from_str(&m.peer).expect("Invalid peer pubkey"),
            confirmations_required: m.confirmations_required,
            confirmations: m.confirmations,
            is_outbound: m.is_outbound,
            is_usable: m.is_usable,
            is_anchor: m.is_anchor,
        }
    }
}

/// Information about a channel that was closed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[wasm_bindgen]
pub struct ChannelClosure {
    channel_id: Option<String>,
    node_id: Option<String>,
    reason: String,
    pub timestamp: u64,
}

#[wasm_bindgen]
impl ChannelClosure {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }

    #[wasm_bindgen(getter)]
    pub fn channel_id(&self) -> Option<String> {
        self.channel_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn node_id(&self) -> Option<String> {
        self.node_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn reason(&self) -> String {
        self.reason.clone()
    }
}

impl PartialOrd for ChannelClosure {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ChannelClosure {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

impl From<nodemanager::ChannelClosure> for ChannelClosure {
    fn from(c: nodemanager::ChannelClosure) -> Self {
        ChannelClosure {
            channel_id: c.channel_id.map(|c| c.to_lower_hex_string()),
            node_id: c.node_id.map(|c| c.serialize().to_lower_hex_string()),
            reason: c.reason,
            timestamp: c.timestamp,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[wasm_bindgen]
pub struct MutinyBalance {
    pub confirmed: u64,
    pub unconfirmed: u64,
    pub lightning: u64,
    pub force_close: u64,
}

#[wasm_bindgen]
impl MutinyBalance {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }
}

impl From<mutiny_core::MutinyBalance> for MutinyBalance {
    fn from(m: mutiny_core::MutinyBalance) -> Self {
        MutinyBalance {
            confirmed: m.confirmed,
            unconfirmed: m.unconfirmed,
            lightning: m.lightning,
            force_close: m.force_close,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[wasm_bindgen]
pub struct LnUrlParams {
    pub max: u64,
    pub min: u64,
    tag: String,
}

#[wasm_bindgen]
impl LnUrlParams {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }

    #[wasm_bindgen(getter)]
    pub fn tag(&self) -> String {
        self.tag.clone()
    }
}

impl From<mutiny_core::LnUrlParams> for LnUrlParams {
    fn from(m: mutiny_core::LnUrlParams) -> Self {
        LnUrlParams {
            max: m.max,
            min: m.min,
            tag: m.tag,
        }
    }
}

// This is the NodeIdentity that refer to a specific node
// Used for public facing identification.
#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[wasm_bindgen]
pub struct NodeIdentity {
    uuid: String,
    pubkey: PublicKey,
}

#[wasm_bindgen]
impl NodeIdentity {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }

    #[wasm_bindgen(getter)]
    pub fn uuid(&self) -> String {
        self.uuid.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn pubkey(&self) -> String {
        self.pubkey.to_string()
    }
}

impl From<nodemanager::NodeIdentity> for NodeIdentity {
    fn from(m: nodemanager::NodeIdentity) -> Self {
        NodeIdentity {
            uuid: m.uuid,
            pubkey: m.pubkey,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[wasm_bindgen]
pub struct MutinyBip21RawMaterials {
    pub(crate) address: String,
    pub(crate) invoice: Option<String>,
    pub(crate) btc_amount: Option<String>,
    pub(crate) labels: Vec<String>,
}

#[wasm_bindgen]
impl MutinyBip21RawMaterials {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }

    #[wasm_bindgen(getter)]
    pub fn address(&self) -> String {
        self.address.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn invoice(&self) -> Option<String> {
        self.invoice.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn btc_amount(&self) -> Option<String> {
        self.btc_amount.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn labels(&self) -> Vec<String> {
        self.labels.clone()
    }
}

impl From<nodemanager::MutinyBip21RawMaterials> for MutinyBip21RawMaterials {
    fn from(m: nodemanager::MutinyBip21RawMaterials) -> Self {
        MutinyBip21RawMaterials {
            address: m.address.to_string(),
            invoice: m.invoice.map(|i| i.to_string()),
            btc_amount: m.btc_amount,
            labels: m.labels,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[wasm_bindgen]
pub struct AuthProfile {
    pub index: u32,
    name: String,
    used_services: Vec<String>,
}

#[wasm_bindgen]
impl AuthProfile {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn used_services(&self) -> Vec<String> {
        self.used_services.clone()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq, PartialEq, Hash)]
#[wasm_bindgen]
pub enum TagKind {
    Label,
    Contact,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[wasm_bindgen]
pub struct TagItem {
    id: String,
    pub kind: TagKind,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    npub: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ln_address: Option<LightningAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lnurl: Option<LnUrl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    primal_image_url: Option<String>,
    /// Epoch time in seconds when this tag was last used
    pub last_used_time: u64,
    /// If we follow this npub on nostr
    pub is_followed: bool,
}

impl PartialOrd for TagItem {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TagItem {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        // sort last used time in descending order
        // then sort by name and id
        other
            .last_used_time
            .cmp(&self.last_used_time)
            .then(self.name.cmp(&other.name))
            .then(self.id.cmp(&other.id))
    }
}

#[wasm_bindgen]
impl TagItem {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn npub(&self) -> Option<String> {
        self.npub.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn ln_address(&self) -> Option<String> {
        self.ln_address.clone().map(|a| a.to_string())
    }

    #[wasm_bindgen(getter)]
    pub fn lnurl(&self) -> Option<String> {
        self.lnurl.clone().map(|a| a.to_string())
    }

    #[wasm_bindgen(getter)]
    pub fn image_url(&self) -> Option<String> {
        self.image_url.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn primal_image_url(&self) -> Option<String> {
        self.primal_image_url.clone()
    }
}

impl TagItem {
    pub fn from(id: String, contact: MutinyContact, is_followed: bool) -> Self {
        TagItem {
            id,
            kind: TagKind::Contact,
            name: contact.name,
            npub: contact.npub.map(|n| n.to_bech32().expect("bech32")),
            ln_address: contact.ln_address,
            lnurl: contact.lnurl,
            primal_image_url: contact.image_url.as_ref().map(|i| {
                format!(
                    "https://primal.b-cdn.net/media-cache?s=s&a=1&u={}",
                    urlencoding::encode(i)
                )
            }),
            image_url: contact.image_url,
            last_used_time: contact.last_used,
            is_followed,
        }
    }
}

impl From<labels::TagItem> for TagItem {
    fn from(m: labels::TagItem) -> Self {
        match m {
            labels::TagItem::Label((label, item)) => TagItem {
                id: label.clone(),
                kind: TagKind::Label,
                name: label,
                npub: None,
                ln_address: None,
                lnurl: None,
                image_url: None,
                primal_image_url: None,
                last_used_time: item.last_used_time,
                is_followed: false,
            },
            labels::TagItem::Contact((id, contact)) => TagItem::from(id, contact, false),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[wasm_bindgen]
pub struct NwcProfile {
    name: String,
    pub index: u32,
    /// Maximum amount of sats that can be sent in a single payment
    pub max_single_amt_sats: u64,
    relay: String,
    /// Require approval before sending a payment
    pub require_approval: bool,
    spending_conditions: SpendingConditions,
    nwc_uri: Option<String>,
    tag: String,
    label: Option<String>,
    enabled: bool,
}

impl Serialize for NwcProfile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let json = json!({
            "name": self.name,
            "index": self.index,
            "max_single_amt_sats": self.max_single_amt_sats,
            "relay": self.relay,
            "require_approval": self.require_approval,
            "spending_conditions": json!(self.spending_conditions),
            "nwc_uri": self.nwc_uri,
            "tag": self.tag,
            "label": self.label,
            "budget_amount": self.budget_amount(),
            "budget_period": self.budget_period(),
            "budget_remaining": self.budget_remaining(),
            "active_payments": self._active_payments(),
            "spending_conditions_type": self.spending_conditions_type(),
            "url_suffix": self.url_suffix(),
            "enabled": self.enabled(),
        });

        json.serialize(serializer)
    }
}

#[wasm_bindgen]
impl NwcProfile {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }

    #[wasm_bindgen(getter)]
    pub fn spending_conditions_type(&self) -> String {
        match self.spending_conditions {
            SpendingConditions::SingleUse(_) => "SingleUse".to_string(),
            SpendingConditions::RequireApproval => "RequireApproval".to_string(),
            SpendingConditions::Budget(_) => "Budget".to_string(),
        }
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn relay(&self) -> String {
        self.relay.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn nwc_uri(&self) -> Option<String> {
        self.nwc_uri.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn tag(&self) -> String {
        self.tag.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn label(&self) -> Option<String> {
        self.label.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn budget_amount(&self) -> Option<u64> {
        match &self.spending_conditions {
            SpendingConditions::Budget(budget) => Some(budget.budget),
            SpendingConditions::SingleUse(single) => Some(single.amount_sats),
            SpendingConditions::RequireApproval => None,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn budget_period(&self) -> Option<String> {
        match &self.spending_conditions {
            SpendingConditions::Budget(budget) => match budget.period {
                nostr::nwc::BudgetPeriod::Day => Some("Day".to_string()),
                nostr::nwc::BudgetPeriod::Week => Some("Week".to_string()),
                nostr::nwc::BudgetPeriod::Month => Some("Month".to_string()),
                nostr::nwc::BudgetPeriod::Year => Some("Year".to_string()),
                nostr::nwc::BudgetPeriod::Seconds(secs) => Some(format!("{secs} Seconds")),
            },
            SpendingConditions::SingleUse(_) => None,
            SpendingConditions::RequireApproval => None,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn budget_remaining(&self) -> Option<u64> {
        match &self.spending_conditions {
            SpendingConditions::Budget(budget) => Some(budget.budget_remaining()),
            SpendingConditions::SingleUse(single) => {
                if single.payment_hash.is_some() {
                    Some(0)
                } else {
                    Some(single.amount_sats)
                }
            }
            SpendingConditions::RequireApproval => None,
        }
    }

    fn _active_payments(&self) -> Vec<String> {
        match &self.spending_conditions {
            SpendingConditions::Budget(budget) => {
                budget.payments.iter().map(|x| x.hash.clone()).collect()
            }
            SpendingConditions::SingleUse(_) => vec![],
            SpendingConditions::RequireApproval => vec![],
        }
    }

    #[wasm_bindgen(getter)]
    pub fn active_payments(&self) -> Vec<String> {
        self._active_payments().clone()
    }

    #[wasm_bindgen(getter)]
    pub fn url_suffix(&self) -> Option<String> {
        if let SpendingConditions::SingleUse(ref cond) = self.spending_conditions {
            let encoded =
                urlencoding::encode(self.nwc_uri.as_ref().expect("single use must have uri"));
            Some(format!(
                "/gift?amount={}&nwc_uri={}",
                cond.amount_sats, encoded
            ))
        } else {
            None
        }
    }

    #[wasm_bindgen(getter)]
    pub fn enabled(&self) -> bool {
        self.enabled
    }
}

impl From<nostr::nwc::NwcProfile> for NwcProfile {
    fn from(value: nostr::nwc::NwcProfile) -> Self {
        let (require_approval, max_single_amt_sats) = match value.spending_conditions.clone() {
            SpendingConditions::SingleUse(single) => {
                if single.payment_hash.is_some() {
                    (false, 0)
                } else {
                    (false, single.amount_sats)
                }
            }
            SpendingConditions::RequireApproval => (true, 0),
            SpendingConditions::Budget(budget) => (false, budget.single_max.unwrap_or_default()),
        };

        NwcProfile {
            name: value.name,
            index: value.index,
            relay: value.relay,
            max_single_amt_sats,
            require_approval,
            spending_conditions: value.spending_conditions,
            nwc_uri: value.nwc_uri,
            tag: value.tag.to_string(),
            label: value.label,
            enabled: value.enabled.unwrap_or(true),
        }
    }
}

/// An invoice received over Nostr Wallet Connect that is pending approval or rejection
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[wasm_bindgen]
pub struct PendingNwcInvoice {
    /// Index of the profile that received the invoice.
    /// None if invoice is from a DM
    pub index: Option<u32>,
    /// If this is a DM, this is who sent us the request
    npub: Option<String>,
    /// The invoice that awaiting approval
    invoice: String,
    /// The id of the invoice, this is the payment hash
    id: String,
    /// The amount of sats that the invoice is for
    pub amount_sats: u64,
    /// The description of the invoice
    invoice_description: Option<String>,
    /// Name of the NWC Profile that received the invoice
    profile_name: Option<String>,
    /// Invoice expire time in seconds since epoch
    pub expiry: u64,
}

#[wasm_bindgen]
impl PendingNwcInvoice {
    #[wasm_bindgen(getter)]
    pub fn npub(&self) -> Option<String> {
        self.npub.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn invoice(&self) -> String {
        self.invoice.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn invoice_description(&self) -> Option<String> {
        self.invoice_description.clone()
    }

    /// Name of the NWC Profile that received the invoice
    #[wasm_bindgen(getter)]
    pub fn profile_name(&self) -> Option<String> {
        self.profile_name.clone()
    }
}

impl From<(nostr::nwc::PendingNwcInvoice, Option<String>)> for PendingNwcInvoice {
    fn from((value, profile_name): (nostr::nwc::PendingNwcInvoice, Option<String>)) -> Self {
        let invoice_description = match value.invoice.description() {
            Bolt11InvoiceDescription::Direct(desc) => Some(desc.to_string()),
            Bolt11InvoiceDescription::Hash(_) => None,
        };
        let npub = if profile_name.is_none() {
            Some(value.pubkey.to_bech32().expect("bech32"))
        } else {
            None
        };

        let timestamp = value.invoice.duration_since_epoch().as_secs();
        let expiry = timestamp + value.invoice.expiry_time().as_secs();

        PendingNwcInvoice {
            index: value.index,
            npub,
            invoice: value.invoice.to_string(),
            id: value.invoice.payment_hash().into_32().to_lower_hex_string(),
            amount_sats: value.invoice.amount_milli_satoshis().unwrap_or_default() / 1_000,
            invoice_description,
            profile_name,
            expiry,
        }
    }
}

// This is a subscription plan for Mutiny+
#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[wasm_bindgen]
pub struct Plan {
    pub id: u8,
    pub amount_sat: u64,
}

#[wasm_bindgen]
impl Plan {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }
}

impl From<mutiny_core::Plan> for Plan {
    fn from(m: mutiny_core::Plan) -> Self {
        Plan {
            id: m.id,
            amount_sat: m.amount_sat,
        }
    }
}

/// FedimintSweepResult is the result of how much was swept and the fees paid.
#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[wasm_bindgen]
pub struct FedimintSweepResult {
    pub amount: u64,
    pub fees: Option<u64>,
}

#[wasm_bindgen]
impl FedimintSweepResult {
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> JsValue {
        JsValue::from_serde(&serde_json::to_value(self).unwrap()).unwrap()
    }
}

impl From<mutiny_core::FedimintSweepResult> for FedimintSweepResult {
    fn from(m: mutiny_core::FedimintSweepResult) -> Self {
        FedimintSweepResult {
            amount: m.amount,
            fees: m.fees,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[wasm_bindgen]
pub enum BudgetPeriod {
    Day,
    Week,
    Month,
    Year,
}

impl From<BudgetPeriod> for nostr::nwc::BudgetPeriod {
    fn from(value: BudgetPeriod) -> Self {
        match value {
            BudgetPeriod::Day => Self::Day,
            BudgetPeriod::Week => Self::Week,
            BudgetPeriod::Month => Self::Month,
            BudgetPeriod::Year => Self::Year,
        }
    }
}

impl TryFrom<nostr::nwc::BudgetPeriod> for BudgetPeriod {
    type Error = ();

    fn try_from(value: nostr::nwc::BudgetPeriod) -> Result<Self, Self::Error> {
        match value {
            nostr::nwc::BudgetPeriod::Day => Ok(Self::Day),
            nostr::nwc::BudgetPeriod::Week => Ok(Self::Week),
            nostr::nwc::BudgetPeriod::Month => Ok(Self::Month),
            nostr::nwc::BudgetPeriod::Year => Ok(Self::Year),
            nostr::nwc::BudgetPeriod::Seconds(_) => Err(()),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DirectMessage {
    pub from: String,
    pub to: String,
    pub message: String,
    pub date: u64,
    pub event_id: String,
}

impl From<mutiny_core::DirectMessage> for DirectMessage {
    fn from(value: mutiny_core::DirectMessage) -> Self {
        Self {
            from: value.from.to_bech32().expect("bech32"),
            to: value.to.to_bech32().expect("bech32"),
            message: value.message,
            date: value.date,
            event_id: value.event_id.to_hex(),
        }
    }
}
