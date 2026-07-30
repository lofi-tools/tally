// use serde::{Deserialize, Serialize};
// use std::collections::HashMap;

// // ============================================================
// // FRS-105 Specific Types - Simplified
// // ============================================================

// /// Represents a single FRS-105 computation/line item
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct FRS105Line {
//     pub id: String,
//     pub description: String,
//     pub period: Period,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub value: Option<f64>,
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub enum Period {
//     #[serde(rename = "in-year")]
//     InYear,
//     #[serde(rename = "at-end")]
//     AtEnd,
//     #[serde(rename = "at-start")]
//     AtStart,
// }

// // ============================================================
// // FRS-105 Report Structure - Simple and Direct
// // ============================================================

// #[derive(Debug, Clone, Default)]
// pub struct FRS105Report {
//     // Income Statement (In-Year)
//     pub turnover: Option<f64>,
//     pub other_operating_income: Option<f64>,
//     pub raw_materials: Option<f64>,
//     pub gross_profit: Option<f64>,
//     pub staff_costs: Option<f64>,
//     pub depreciation: Option<f64>,
//     pub other_charges: Option<f64>,
//     pub profit: Option<f64>,

//     // Balance Sheet (At End)
//     pub fixed_assets: Option<f64>,
//     pub current_assets: Option<f64>,
//     pub creditors_within_one_year: Option<f64>,
//     pub net_current_assets: Option<f64>,
//     pub total_assets_less_liabilities: Option<f64>,
//     pub creditors_after_one_year: Option<f64>,
//     pub provisions_for_liabilities: Option<f64>,
//     pub net_assets: Option<f64>,

//     // Equity (At End)
//     pub share_capital: Option<f64>,
//     pub retained_profit: Option<f64>,
//     pub dividends: Option<f64>,
//     pub corporation_tax: Option<f64>,
//     pub total_equity: Option<f64>,

//     // Cash Flow (In-Year)
//     pub cash_at_start: Option<f64>,
//     pub cash_from_operations: Option<f64>,
//     pub cash_from_investing: Option<f64>,
//     pub cash_from_financing: Option<f64>,
//     pub cash_at_end: Option<f64>,

//     // Fixed Assets Note
//     pub fixed_assets_at_start: Option<f64>,
//     pub fixed_assets_additions: Option<f64>,
//     pub depreciation_at_start: Option<f64>,
//     pub depreciation_charge: Option<f64>,
//     pub carrying_amount: Option<f64>,
// }

// // ============================================================
// // FRS-105 Mapper - Hardcoded Logic
// // ============================================================

// pub struct FRS105Mapper {
//     // GnuCash account balances
//     account_balances: HashMap<String, f64>,
// }

// #[allow(clippy::new_without_default)]
// impl FRS105Mapper {
//     pub fn new() -> Self {
//         Self {
//             account_balances: HashMap::new(),
//         }
//     }

//     /// Set a GnuCash account balance
//     pub fn set_account_balance(&mut self, account_path: &str, balance: f64) {
//         self.account_balances
//             .insert(account_path.to_string(), balance);
//     }

//     /// Set multiple account balances at once
//     pub fn set_account_balances(&mut self, balances: HashMap<String, f64>) {
//         self.account_balances.extend(balances);
//     }

//     /// Get an account balance (returns 0.0 if not found)
//     fn get_balance(&self, account_path: &str) -> f64 {
//         self.account_balances
//             .get(account_path)
//             .copied()
//             .unwrap_or(0.0)
//     }

//     /// Sum multiple account balances
//     fn sum_accounts(&self, accounts: &[&str]) -> f64 {
//         accounts.iter().map(|&acc| self.get_balance(acc)).sum()
//     }

//     /// Map GnuCash accounts to FRS-105 line items
//     pub fn map(&self) -> FRS105Report {
//         let mut report = FRS105Report::default();

//         // ============================================================
//         // Income Statement (In-Year)
//         // ============================================================

//         // Turnover / Revenue
//         let main_income = self.sum_accounts(&["Income:Sales"]);
//         report.turnover = Some(main_income);

//         // Other operating income
//         report.other_operating_income = Some(0.0); // No specific accounts mapped

//         // Raw materials
//         report.raw_materials = Some(0.0); // No specific accounts mapped

//         // Gross Profit (turnover - other income - raw materials)
//         report.gross_profit = Some(
//             report.turnover.unwrap_or(0.0) + report.other_operating_income.unwrap_or(0.0)
//                 - report.raw_materials.unwrap_or(0.0),
//         );

//         // Staff costs
//         let salaries = self.sum_accounts(&[
//             "Expenses:Emoluments:Employees",
//             "Expenses:Emoluments:Employer's NICs",
//         ]);
//         let pensions = self.get_balance("Expenses:Emoluments:Employer Pension Contribution");
//         report.staff_costs = Some(salaries + pensions);

//         // Depreciation
//         report.depreciation = Some(self.get_balance("Expenses:Depreciation"));

//         // Other charges (sum of various expenses)
//         let other_charges = self.sum_accounts(&[
//             "Expenses:VAT Purchases:Accountant",
//             "Expenses:VAT Purchases:Bank Charges",
//             "Expenses:VAT Purchases:Office",
//             "Expenses:VAT Purchases:Software",
//             "Expenses:VAT Purchases:Subscriptions",
//             "Expenses:VAT Purchases:Sundries",
//             "Expenses:VAT Purchases:Telecoms",
//             "Expenses:VAT Purchases:Travel/Accom",
//         ]);
//         report.other_charges = Some(other_charges);

//         // Profit
//         report.profit = Some(
//             report.turnover.unwrap_or(0.0)
//                 + report.other_operating_income.unwrap_or(0.0)
//                 + report.raw_materials.unwrap_or(0.0)
//                 + report.staff_costs.unwrap_or(0.0)
//                 + report.depreciation.unwrap_or(0.0)
//                 + report.other_charges.unwrap_or(0.0),
//         );

//         // ============================================================
//         // Balance Sheet (At End)
//         // ============================================================

//         // Fixed Assets
//         report.fixed_assets = Some(self.get_balance("Assets:Capital Equipment"));

//         // Current Assets
//         let debtors = self.sum_accounts(&["Accounts Receivable", "Assets:Owed To Us"]);
//         let vat_refund = self.sum_accounts(&[
//             "VAT:Input",
//             "VAT:Settlement:Input",
//             "Assets:VAT Repayments Due",
//         ]);
//         let bank = self.get_balance("Bank Accounts");
//         report.current_assets = Some(debtors + vat_refund + bank);

//         // Creditors within one year
//         let trade_creditors = self.get_balance("Accounts Payable");
//         let other_creditors = self.sum_accounts(&[
//             "VAT:Output",
//             "VAT:Settlement:Output",
//             "Liabilities:Credit Cards",
//             "Liabilities:Owed Corporation Tax",
//         ]);
//         report.creditors_within_one_year = Some(trade_creditors + other_creditors);

//         // Net Current Assets
//         report.net_current_assets = Some(
//             report.current_assets.unwrap_or(0.0) + report.creditors_within_one_year.unwrap_or(0.0),
//         );

//         // Total Assets Less Liabilities
//         report.total_assets_less_liabilities = Some(
//             report.fixed_assets.unwrap_or(0.0)
//                 + report.current_assets.unwrap_or(0.0)
//                 + report.creditors_within_one_year.unwrap_or(0.0),
//         );

//         // Creditors after one year - none mapped
//         report.creditors_after_one_year = Some(0.0);

//         // Provisions - none mapped
//         report.provisions_for_liabilities = Some(0.0);

//         // Net Assets
//         report.net_assets = Some(
//             report.total_assets_less_liabilities.unwrap_or(0.0)
//                 - report.creditors_after_one_year.unwrap_or(0.0)
//                 - report.provisions_for_liabilities.unwrap_or(0.0),
//         );

//         // ============================================================
//         // Equity
//         // ============================================================

//         report.share_capital = Some(self.get_balance("Equity:Shareholdings"));
//         report.retained_profit = Some(self.sum_accounts(&["Income", "Expenses"]));
//         report.dividends = Some(self.get_balance("Equity:Dividends"));
//         report.corporation_tax = Some(self.get_balance("Equity:Corporation Tax"));

//         report.total_equity = Some(
//             report.share_capital.unwrap_or(0.0)
//                 + report.retained_profit.unwrap_or(0.0)
//                 + report.dividends.unwrap_or(0.0)
//                 + report.corporation_tax.unwrap_or(0.0),
//         );

//         // ============================================================
//         // Cash Flow
//         // ============================================================

//         report.cash_at_start = Some(self.get_balance("Bank Accounts"));

//         // Cash from operations
//         let cash_inflow = report.profit.unwrap_or(0.0);
//         let depreciation_adjustment = self.get_balance("Expenses:Depreciation");
//         let receivables_change = self.sum_accounts(&[
//             "Accounts Receivable",
//             "Assets:Owed To Us",
//             "VAT:Input",
//             "VAT:Settlement:Input",
//         ]);
//         let payables_change = self.sum_accounts(&[
//             "Accounts Payable",
//             "VAT:Output",
//             "VAT:Settlement:Output",
//             "Liabilities:Owed Corporation Tax",
//         ]);

//         report.cash_from_operations =
//             Some(cash_inflow + depreciation_adjustment - receivables_change + payables_change);

//         // Cash from investing
//         let sale_equipment = self.get_balance("Assets:Capital Equipment:Computer Equipment"); // Negative
//         let purchase_equipment = -self.get_balance("Assets:Capital Equipment:Computer Equipment");
//         let interest_received = self.get_balance("Income:Interest");

//         report.cash_from_investing = Some(sale_equipment + purchase_equipment + interest_received);

//         // Cash from financing
//         let interest_paid = self.get_balance("Expenses:Interest Paid");
//         let shares_issued = 0.0;
//         let dividends_paid = self.get_balance("Equity:Dividends");

//         report.cash_from_financing = Some(interest_paid + shares_issued + dividends_paid);

//         // Cash at end
//         report.cash_at_end = Some(
//             report.cash_at_start.unwrap_or(0.0)
//                 + report.cash_from_operations.unwrap_or(0.0)
//                 + report.cash_from_investing.unwrap_or(0.0)
//                 + report.cash_from_financing.unwrap_or(0.0),
//         );

//         // ============================================================
//         // Fixed Assets Note
//         // ============================================================

//         report.fixed_assets_at_start =
//             Some(self.get_balance("Assets:Capital Equipment:Computer Equipment"));
//         report.fixed_assets_additions =
//             Some(self.get_balance("Assets:Capital Equipment:Computer Equipment"));
//         report.depreciation_at_start =
//             Some(self.get_balance("Assets:Capital Equipment:Depreciation"));
//         report.depreciation_charge =
//             Some(self.get_balance("Assets:Capital Equipment:Depreciation"));
//         report.carrying_amount = Some(
//             report.fixed_assets_at_start.unwrap_or(0.0)
//                 + report.fixed_assets_additions.unwrap_or(0.0)
//                 - report.depreciation_at_start.unwrap_or(0.0)
//                 - report.depreciation_charge.unwrap_or(0.0),
//         );

//         report
//     }
// }

// // ============================================================
// // Simplified Usage Example
// // ============================================================

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_frs105_mapper() {
//         let mut mapper = FRS105Mapper::new();

//         // Set up GnuCash account balances
//         let mut balances = HashMap::new();
//         balances.insert("Income:Sales".to_string(), 100000.0);
//         balances.insert("Expenses:Emoluments:Employees".to_string(), -40000.0);
//         balances.insert("Expenses:Emoluments:Employer's NICs".to_string(), -5000.0);
//         balances.insert(
//             "Expenses:Emoluments:Employer Pension Contribution".to_string(),
//             -3000.0,
//         );
//         balances.insert("Expenses:Depreciation".to_string(), -5000.0);
//         balances.insert("Expenses:VAT Purchases:Accountant".to_string(), -1000.0);
//         balances.insert("Expenses:VAT Purchases:Bank Charges".to_string(), -500.0);
//         balances.insert("Expenses:VAT Purchases:Office".to_string(), -2000.0);
//         balances.insert("Assets:Capital Equipment".to_string(), 50000.0);
//         balances.insert("Bank Accounts".to_string(), 25000.0);
//         balances.insert("Equity:Shareholdings".to_string(), 10000.0);
//         balances.insert("Accounts Receivable".to_string(), 15000.0);
//         balances.insert("Accounts Payable".to_string(), -8000.0);
//         balances.insert("VAT:Output".to_string(), -2000.0);
//         balances.insert("Income:Interest".to_string(), 200.0);
//         balances.insert("Expenses:Interest Paid".to_string(), -300.0);
//         balances.insert("Equity:Dividends".to_string(), -5000.0);

//         mapper.set_account_balances(balances);

//         let report = mapper.map();

//         // Test income statement
//         assert_eq!(report.turnover, Some(100000.0));
//         assert_eq!(report.staff_costs, Some(-48000.0)); // -40000 -5000 -3000
//         assert_eq!(report.depreciation, Some(-5000.0));
//         assert_eq!(report.other_charges, Some(-3500.0)); // -1000 -500 -2000
//         assert!(report.profit.unwrap_or(0.0) > 0.0); // 100000 + 0 + 0 + (-48000) + (-5000) + (-3500) = 43500

//         // Test balance sheet
//         assert_eq!(report.fixed_assets, Some(50000.0));
//         assert_eq!(report.current_assets, Some(40000.0)); // 25000 + 15000
//         assert_eq!(report.creditors_within_one_year, Some(-10000.0)); // -8000 + -2000
//         assert_eq!(report.net_current_assets, Some(30000.0)); // 40000 - 10000

//         // Test equity
//         assert_eq!(report.share_capital, Some(10000.0));
//         assert!(report.total_equity.unwrap_or(0.0) > 0.0);

//         // Test cash flow
//         assert_eq!(report.cash_at_start, Some(25000.0));
//         assert!(report.cash_from_operations.unwrap_or(0.0) > 0.0); // Operating cash flow
//         assert_eq!(report.cash_from_investing, Some(200.0)); // Interest only
//         assert_eq!(report.cash_from_financing, Some(-5300.0)); // -300 -5000 + 0
//     }

//     #[test]
//     fn test_mapper_with_minimal_balances() {
//         let mapper = FRS105Mapper::new();
//         let report = mapper.map();

//         // All values should be 0.0 when no balances are set
//         assert_eq!(report.turnover, Some(0.0));
//         assert_eq!(report.profit, Some(0.0));
//         assert_eq!(report.net_assets, Some(0.0));
//         assert_eq!(report.cash_at_end, Some(0.0));
//     }

//     #[test]
//     fn test_mapper_with_specific_accounts() {
//         let mut mapper = FRS105Mapper::new();

//         // Test specific account mapping
//         mapper.set_account_balance("Income:Sales", 50000.0);
//         mapper.set_account_balance("Expenses:Emoluments:Employees", -20000.0);
//         mapper.set_account_balance("Bank Accounts", 10000.0);

//         let report: FRS105Report = mapper.map();

//         assert_eq!(report.turnover, Some(50000.0));
//         assert_eq!(report.staff_costs, Some(-20000.0));
//         assert_eq!(report.cash_at_start, Some(10000.0));
//         assert_eq!(report.net_assets, Some(10000.0));
//     }
// }

// // ============================================================
// // Display/Output Helpers
// // ============================================================

// impl FRS105Report {
//     /// Format as a simple text report
//     pub fn to_text(&self) -> String {
//         let mut output = String::new();

//         output.push_str("=== FRS-105 Micro-Entity Accounts ===\n\n");

//         output.push_str("Income Statement (In-Year)\n");
//         output.push_str("---------------------------\n");
//         output.push_str(&format!(
//             "Turnover:              £{:>12.2}\n",
//             self.turnover.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Other Operating Income: £{:>12.2}\n",
//             self.other_operating_income.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Raw Materials:          £{:>12.2}\n",
//             self.raw_materials.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Gross Profit:           £{:>12.2}\n",
//             self.gross_profit.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Staff Costs:            £{:>12.2}\n",
//             self.staff_costs.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Depreciation:           £{:>12.2}\n",
//             self.depreciation.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Other Charges:          £{:>12.2}\n",
//             self.other_charges.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Profit (Loss):          £{:>12.2}\n",
//             self.profit.unwrap_or(0.0)
//         ));

//         output.push_str("\nBalance Sheet (At End)\n");
//         output.push_str("-----------------------\n");
//         output.push_str(&format!(
//             "Fixed Assets:           £{:>12.2}\n",
//             self.fixed_assets.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Current Assets:         £{:>12.2}\n",
//             self.current_assets.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Creditors (<1yr):       £{:>12.2}\n",
//             self.creditors_within_one_year.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Net Current Assets:     £{:>12.2}\n",
//             self.net_current_assets.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Total Assets Less Liab: £{:>12.2}\n",
//             self.total_assets_less_liabilities.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Creditors (>1yr):       £{:>12.2}\n",
//             self.creditors_after_one_year.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Provisions:             £{:>12.2}\n",
//             self.provisions_for_liabilities.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Net Assets:             £{:>12.2}\n",
//             self.net_assets.unwrap_or(0.0)
//         ));

//         output.push_str("\nEquity\n");
//         output.push_str("------\n");
//         output.push_str(&format!(
//             "Share Capital:          £{:>12.2}\n",
//             self.share_capital.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Retained Profit:        £{:>12.2}\n",
//             self.retained_profit.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Dividends:              £{:>12.2}\n",
//             self.dividends.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Corporation Tax:        £{:>12.2}\n",
//             self.corporation_tax.unwrap_or(0.0)
//         ));
//         output.push_str(&format!(
//             "Total Equity:           £{:>12.2}\n",
//             self.total_equity.unwrap_or(0.0)
//         ));

//         output
//     }
// }

// // // ============================================================
// // // Main function example
// // // ============================================================

// // fn main() {
// //     let mut mapper = FRS105Mapper::new();

// //     // Load account balances from your data source
// //     let balances = load_account_balances_from_gnucash();
// //     mapper.set_account_balances(balances);

// //     // Generate the FRS-105 report
// //     let report = mapper.map();

// //     // Print the report
// //     println!("{}", report.to_text());

// //     // Or export to JSON
// //     // let json = serde_json::to_string_pretty(&report).unwrap();
// //     // std::fs::write("frs105_report.json", json).unwrap();
// // }

// // fn load_account_balances_from_gnucash() -> HashMap<String, f64> {
// //     // In reality, this would read from a GnuCash SQLite database or XML file
// //     // For now, return some example data
// //     let mut balances = HashMap::new();
// //     balances.insert("Income:Sales".to_string(), 150000.0);
// //     balances.insert("Expenses:Emoluments:Employees".to_string(), -60000.0);
// //     // ... etc
// //     balances
// // }
