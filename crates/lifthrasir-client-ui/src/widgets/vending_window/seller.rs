//! Seller draft: the unsent listings and their local pre-flight validation.

use std::collections::HashSet;

use bevy::prelude::*;
use lifthrasir_client::domain::cart::Cart;
use net_contract::dto::{VendingOffer, VendingSetup};

/// One listed cart slot with its drafted quantity and price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftRow {
    pub cart_index: u32,
    pub amount: u32,
    pub price: u32,
}

/// The seller's unsent listings. Holds no authoritative inventory or zeny.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct SellerDraft {
    pub title: String,
    pub rows: Vec<DraftRow>,
}

impl SellerDraft {
    /// The listings as they will be submitted, in draft order.
    pub fn offers(&self) -> Vec<VendingOffer> {
        self.rows
            .iter()
            .map(|row| VendingOffer {
                cart_index: row.cart_index,
                amount: row.amount,
                price: row.price,
            })
            .collect()
    }

    pub fn row(&self, cart_index: u32) -> Option<&DraftRow> {
        self.rows.iter().find(|row| row.cart_index == cart_index)
    }

    /// Lists one sellable slot with its whole stack at the minimum price.
    ///
    /// Returns `Ok(false)` when the slot is already listed, and an error for a
    /// slot the setup does not allow or a full shop.
    pub fn add(
        &mut self,
        cart_index: u32,
        stock: u32,
        setup: &VendingSetup,
    ) -> Result<bool, String> {
        if !setup.sellable_cart_indices.contains(&cart_index) {
            return Err("That cart slot cannot be sold.".to_string());
        }
        if self.row(cart_index).is_some() {
            return Ok(false);
        }
        if self.rows.len() >= setup.max_slots as usize {
            return Err(format!(
                "A shop can list at most {} items.",
                setup.max_slots
            ));
        }
        self.rows.push(DraftRow {
            cart_index,
            amount: stock,
            price: setup.min_unit_price,
        });
        Ok(true)
    }

    /// Unlists one slot; returns whether it was listed.
    pub fn remove(&mut self, cart_index: u32) -> bool {
        let before = self.rows.len();
        self.rows.retain(|row| row.cart_index != cart_index);
        self.rows.len() != before
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Local pre-flight only: the server validates again at open time.
    pub fn validate(&self, setup: &VendingSetup, cart: &Cart) -> Result<(), String> {
        let title = self.title.trim();
        if title.is_empty() {
            return Err("Enter a shop title.".to_string());
        }
        if title.chars().any(char::is_control) {
            return Err("The shop title cannot contain control characters.".to_string());
        }
        if title.len() > setup.max_title_bytes as usize {
            return Err(format!(
                "The shop title must fit in {} bytes.",
                setup.max_title_bytes
            ));
        }
        if self.rows.is_empty() {
            return Err("List at least one item.".to_string());
        }
        if self.rows.len() > setup.max_slots as usize {
            return Err(format!(
                "A shop can list at most {} items.",
                setup.max_slots
            ));
        }

        let mut seen = HashSet::new();
        for row in &self.rows {
            if !seen.insert(row.cart_index) {
                return Err("Each cart slot can be listed only once.".to_string());
            }
            if !setup.sellable_cart_indices.contains(&row.cart_index) {
                return Err("That cart slot cannot be sold.".to_string());
            }
            let Ok(index) = u16::try_from(row.cart_index) else {
                return Err("That cart slot does not exist.".to_string());
            };
            let Some(item) = cart.get(index) else {
                return Err("That cart slot no longer holds an item.".to_string());
            };
            if row.amount == 0 || row.amount > item.amount {
                return Err(format!("Quantity must be between 1 and {}.", item.amount));
            }
            if row.price < setup.min_unit_price {
                return Err(format!("Price must be at least {}.", setup.min_unit_price));
            }
            if row.price > setup.max_unit_price {
                return Err(format!("Price must be at most {}.", setup.max_unit_price));
            }
        }
        Ok(())
    }
}

/// Truncates to `max_bytes` on a character boundary, so the submitted title is
/// always valid UTF-8 and never longer than the negotiated limit.
pub fn clamp_title(title: &str, max_bytes: u32) -> String {
    let max_bytes = max_bytes as usize;
    if title.len() <= max_bytes {
        return title.to_string();
    }
    let mut end = 0;
    for (index, ch) in title.char_indices() {
        let next = index + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    title[..end].to_string()
}
