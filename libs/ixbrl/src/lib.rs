use std::fmt;

pub struct AccountNode {
    name: String,
    account_type: String,
    balance: rucash::Num,
    children: Vec<AccountNode>,
}

pub struct GnucashBook {
    accounts: Vec<AccountNode>,
    net_assets: rucash::Num,
}

impl AccountNode {
    fn is_zero(&self) -> bool {
        self.balance == rucash::Num::from(0)
    }

    fn has_nonzero_descendant(&self) -> bool {
        !self.children.is_empty()
            && self.children.iter().any(|c| !c.is_zero() || c.has_nonzero_descendant())
    }
}

fn write_tree(f: &mut fmt::Formatter<'_>, nodes: &[AccountNode], depth: usize) -> fmt::Result {
    let mut sorted: Vec<&AccountNode> = nodes.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    for node in &sorted {
        if node.account_type == "ROOT" {
            write_tree(f, &node.children, depth)?;
            continue;
        }
        if node.is_zero() && !node.has_nonzero_descendant() {
            continue;
        }
        let prefix = "  ".repeat(depth);
        writeln!(f, "{prefix}{}: {}", node.name, node.balance)?;
        write_tree(f, &node.children, depth + 1)?;
    }
    Ok(())
}

impl fmt::Display for GnucashBook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_tree(f, &self.accounts, 0)?;
        writeln!(f, "---")?;
        write!(f, "Net assets: {}", self.net_assets)
    }
}

impl GnucashBook {
    pub async fn try_from_book(book: &rucash::Book<rucash::XMLQuery>) -> Result<Self, rucash::Error> {
        let accounts = book.accounts().await?;

        let mut guid_to_idx: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (i, acc) in accounts.iter().enumerate() {
            guid_to_idx.insert(acc.guid.clone(), i);
        }

        let mut balances = vec![rucash::Num::from(0); accounts.len()];
        for (i, acc) in accounts.iter().enumerate() {
            balances[i] = acc.balance(book).await?;
        }

        let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); accounts.len()];
        let mut roots = Vec::new();
        for (i, acc) in accounts.iter().enumerate() {
            match guid_to_idx.get(&acc.parent_guid) {
                Some(&parent_idx) => children_of[parent_idx].push(i),
                None => roots.push(i),
            }
        }

        fn build(
            accounts: &[rucash::model::Account<rucash::XMLQuery>],
            balances: &[rucash::Num],
            children_of: &[Vec<usize>],
            idx: usize,
        ) -> AccountNode {
            AccountNode {
                name: accounts[idx].name.clone(),
                account_type: accounts[idx].r#type.clone(),
                balance: balances[idx].clone(),
                children: children_of[idx]
                    .iter()
                    .map(|&child| build(accounts, balances, children_of, child))
                    .collect(),
            }
        }

        let tree: Vec<AccountNode> = roots
            .iter()
            .map(|&idx| build(&accounts, &balances, &children_of, idx))
            .collect();

        let balance_sheet_types: &[&str] = &[
            "ASSET", "BANK", "CASH", "RECEIVABLE", "LIABILITY", "CREDIT", "PAYABLE",
        ];

        let top_level: Vec<usize> = roots
            .iter()
            .flat_map(|&idx| &children_of[idx])
            .copied()
            .collect();

        let net_assets: rucash::Num = top_level
            .iter()
            .map(|&idx| &accounts[idx])
            .zip(top_level.iter().map(|&idx| &balances[idx]))
            .filter(|(acc, _)| balance_sheet_types.contains(&acc.r#type.as_str()))
            .map(|(_, bal)| bal.clone())
            .sum();

        Ok(GnucashBook {
            accounts: tree,
            net_assets,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rucash::{Book, XMLQuery};

    #[tokio::test]
    async fn parse_gnucash() {
        let query =
            XMLQuery::new("example_data/example.gnucash").expect("failed to open gnucash file");
        let book = Book::new(query).await.expect("failed to create book");
        let gnucash = GnucashBook::try_from_book(&book)
            .await
            .expect("failed to build gnucash book");
        println!("{gnucash}");
        assert_eq!(gnucash.net_assets, rucash::Num::try_from("16225.08").unwrap());
    }
}
