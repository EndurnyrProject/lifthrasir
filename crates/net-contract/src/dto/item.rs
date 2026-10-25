//! Protocol-neutral item instance data shared by inventory, cart and storage.

/// One random option slot of an item: the option id (an index into the client's
/// option table), its rolled value and its parameter. A slot with id `0` is
/// empty; slots after the last populated one are omitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemOption {
    pub id: i32,
    pub value: i32,
    pub param: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_option_compares_by_every_field() {
        let option = ItemOption {
            id: 3,
            value: -2,
            param: 1,
        };

        assert_eq!(
            option,
            ItemOption {
                id: 3,
                value: -2,
                param: 1
            }
        );
        assert_ne!(option, ItemOption { value: 0, ..option });
    }
}
