//! Unit tests for VC Vault contract.

use crate::contract::{VcVaultContract, VcVaultContractClient};
use crate::types::VCStatus;
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Events},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, String, Symbol,
};

// --- Mock factory ---
//
// After the fee-architecture refactor the vault no longer owns fee config.
// At issuance it cross-calls the factory's `quote_fee(issuer) -> FeeQuote` and
// transfers the fee. Vault unit tests register the vault standalone, so they
// must supply a mock factory that answers `quote_fee` (and `is_vault`).

#[derive(Clone)]
#[contracttype]
pub struct FeeQuote {
    pub enabled: bool,
    pub amount: i128,
    pub token: Option<Address>,
    pub dest: Option<Address>,
}

#[contract]
pub struct MockFactory;

#[contractimpl]
impl MockFactory {
    pub fn set_quote(
        e: Env,
        enabled: bool,
        amount: i128,
        token: Option<Address>,
        dest: Option<Address>,
    ) {
        e.storage().instance().set(
            &Symbol::new(&e, "q"),
            &FeeQuote {
                enabled,
                amount,
                token,
                dest,
            },
        );
    }
    pub fn quote_fee(e: Env, _issuer: Address) -> FeeQuote {
        e.storage()
            .instance()
            .get(&Symbol::new(&e, "q"))
            .unwrap_or(FeeQuote {
                enabled: false,
                amount: 0,
                token: None,
                dest: None,
            })
    }
    pub fn is_vault(_e: Env, _addr: Address) -> bool {
        true
    }
}

fn setup() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    Address,
    VcVaultContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let owner = Address::generate(&env);
    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);
    let factory = env.register(MockFactory, ());
    let did_uri = String::from_str(&env, "did:pkh:stellar:testnet:OWNER");
    let contract_id = env.register(
        VcVaultContract,
        (owner.clone(), admin.clone(), did_uri, factory.clone()),
    );
    let client = VcVaultContractClient::new(&env, &contract_id);
    // Issuance is open: no pre-authorization needed.
    (env, owner, admin, issuer, factory, contract_id, client)
}

#[test]
fn test_version() {
    let (_env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    let v = client.version();
    assert!(!v.is_empty());
}

#[test]
fn test_nominate_and_accept_admin() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    let new_admin = Address::generate(&env);
    client.nominate_admin(&new_admin);
    client.accept_contract_admin();
    let another_admin = Address::generate(&env);
    client.nominate_admin(&another_admin);
    client.accept_contract_admin();
}

#[test]
fn test_set_vault_admin() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    let new_admin = Address::generate(&env);
    client.set_vault_admin(&new_admin);
    // The new admin can exercise an admin-gated entrypoint.
    let issuer = Address::generate(&env);
    client.deny_issuer(&issuer);
}

#[test]
fn test_owner_can_correct_did() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    let new_did = String::from_str(&env, "did:stellar:testnet:corrected");
    client.set_vault_did(&new_did);
    assert_eq!(client.vault_did(), Some(new_did));
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")] // VaultRevoked
fn test_set_vault_did_blocked_on_revoked_vault() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    client.revoke_vault();
    client.set_vault_did(&String::from_str(&env, "did:stellar:testnet:x"));
}

#[test]
fn test_vault_owner_getter() {
    let (_env, owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    assert_eq!(client.vault_owner(), owner);
}

#[test]
fn test_revoke_vault() {
    let (_env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    client.revoke_vault();
}

#[test]
#[should_panic]
fn test_issue_after_revoke_vault_panics() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    client.revoke_vault();
    let vc_id = String::from_str(&env, "vc-1");
    let vc_data = String::from_str(&env, "<ciphertext>");
    let issuer_did = String::from_str(&env, "did:pkh:stellar:testnet:ISSUER");
    client.issue(&vc_id, &vc_data, &contract_id, &issuer, &issuer_did);
}

#[test]
fn test_list_vc_ids_empty() {
    let (_env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    assert_eq!(client.list_vc_ids(&0_u32, &200_u32).len(), 0);
}

#[test]
fn test_get_vc_none_for_missing() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    let vc_id = String::from_str(&env, "nonexistent");
    assert!(client.get_vc(&vc_id).is_none());
}

#[test]
fn test_verify_vc_invalid_when_not_in_vault() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    let vc_id = String::from_str(&env, "nonexistent");
    assert_eq!(client.verify_vc(&vc_id), VCStatus::Invalid);
}

#[test]
fn test_vault_authorize_and_store_and_list_and_get() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let vc_id = String::from_str(&env, "vc-1");
    let vc_data = String::from_str(&env, "<ciphertext>");
    let issuer_did = String::from_str(&env, "did:pkh:stellar:testnet:ISSUER");
    client.issue(&vc_id, &vc_data, &contract_id, &issuer, &issuer_did);
    assert_eq!(client.list_vc_ids(&0_u32, &200_u32).len(), 1);
    assert_eq!(client.get_vc(&vc_id).unwrap().data, vc_data);
}

#[test]
fn test_issue_verify_revoke_flow_local_vault() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let vc_id = String::from_str(&env, "vc-123");
    let vc_data = String::from_str(&env, "<ciphertext>");
    let issuer_did = String::from_str(&env, "did:pkh:stellar:testnet:ISSUER");
    client.issue(&vc_id, &vc_data, &contract_id, &issuer, &issuer_did);
    assert_eq!(client.verify_vc(&vc_id), VCStatus::Valid);
    let date = String::from_str(&env, "2025-12-18T00:00:00Z");
    client.revoke(&vc_id, &date);
    assert_eq!(client.verify_vc(&vc_id), VCStatus::Revoked(date));
}

#[test]
fn test_issue_returns_vc_id() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let vc_id = String::from_str(&env, "vc-return");
    let vc_data = String::from_str(&env, "<ciphertext>");
    let issuer_did = String::from_str(&env, "did:pkh:stellar:testnet:ISSUER");
    let returned = client.issue(&vc_id, &vc_data, &contract_id, &issuer, &issuer_did);
    assert_eq!(returned, vc_id);
}

#[test]
#[should_panic]
fn test_issue_invalid_vault_contract_panics() {
    let (env, _owner, _admin, issuer, _factory, _contract_id, client) = setup();
    let wrong_contract = Address::generate(&env);
    let vc_id = String::from_str(&env, "vc-1");
    let vc_data = String::from_str(&env, "<ciphertext>");
    let issuer_did = String::from_str(&env, "did:pkh:stellar:testnet:ISSUER");
    client.issue(&vc_id, &vc_data, &wrong_contract, &issuer, &issuer_did);
}

#[test]
#[should_panic]
fn test_revoke_nonexistent_vc_panics() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    let vc_id = String::from_str(&env, "nonexistent");
    let date = String::from_str(&env, "2025-12-18T00:00:00Z");
    client.revoke(&vc_id, &date);
}

// --- Fee + strict-authorization behavior ---

#[test]
fn test_issue_charges_fee_to_dest() {
    let (env, _owner, _admin, issuer, factory, contract_id, client) = setup();
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_addr = sac.address();
    StellarAssetClient::new(&env, &token_addr).mint(&issuer, &100_000_000);
    let dest = Address::generate(&env);

    let mf = MockFactoryClient::new(&env, &factory);
    mf.set_quote(
        &true,
        &10_000_000_i128,
        &Some(token_addr.clone()),
        &Some(dest.clone()),
    );

    client.issue(
        &String::from_str(&env, "vc-1"),
        &String::from_str(&env, "<data>"),
        &contract_id,
        &issuer,
        &String::from_str(&env, "did:issuer"),
    );
    assert_eq!(
        TokenClient::new(&env, &token_addr).balance(&dest),
        10_000_000
    );
}

#[test]
fn test_issue_no_fee_when_disabled() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    // mock factory default quote is disabled -> issuance succeeds, nothing charged
    client.issue(
        &String::from_str(&env, "vc-1"),
        &String::from_str(&env, "<data>"),
        &contract_id,
        &issuer,
        &String::from_str(&env, "did:issuer"),
    );
    assert_eq!(
        client.verify_vc(&String::from_str(&env, "vc-1")),
        VCStatus::Valid
    );
}

#[test]
fn test_any_issuer_can_issue_open() {
    let (env, _owner, _admin, _issuer, _factory, contract_id, client) = setup();
    let stranger = Address::generate(&env);
    client.issue(
        &String::from_str(&env, "vc-open"),
        &String::from_str(&env, "<data>"),
        &contract_id,
        &stranger,
        &String::from_str(&env, "did:issuer"),
    );
    assert_eq!(
        client.verify_vc(&String::from_str(&env, "vc-open")),
        VCStatus::Valid
    );
}

#[test]
#[should_panic]
fn test_denied_issuer_cannot_issue() {
    let (env, _owner, _admin, _issuer, _factory, contract_id, client) = setup();
    let spammer = Address::generate(&env);
    client.deny_issuer(&spammer);
    client.issue(
        &String::from_str(&env, "vc-x"),
        &String::from_str(&env, "<data>"),
        &contract_id,
        &spammer,
        &String::from_str(&env, "did:issuer"),
    );
}

#[test]
fn test_allow_issuer_reenables_issue() {
    let (env, _owner, _admin, _issuer, _factory, contract_id, client) = setup();
    let iss = Address::generate(&env);
    client.deny_issuer(&iss);
    assert_eq!(client.denied_issuer_count(), 1);
    client.allow_issuer(&iss);
    assert_eq!(client.denied_issuer_count(), 0);
    client.issue(
        &String::from_str(&env, "vc-ok"),
        &String::from_str(&env, "<data>"),
        &contract_id,
        &iss,
        &String::from_str(&env, "did:issuer"),
    );
    assert_eq!(
        client.verify_vc(&String::from_str(&env, "vc-ok")),
        VCStatus::Valid
    );
}

#[test]
fn test_batch_issue_charges_fee_times_n() {
    let (env, _owner, _admin, issuer, factory, contract_id, client) = setup();
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_addr = sac.address();
    StellarAssetClient::new(&env, &token_addr).mint(&issuer, &100_000_000);
    let dest = Address::generate(&env);
    MockFactoryClient::new(&env, &factory).set_quote(
        &true,
        &10_000_000_i128,
        &Some(token_addr.clone()),
        &Some(dest.clone()),
    );

    let vcs = vec![
        &env,
        (String::from_str(&env, "a"), String::from_str(&env, "<d>")),
        (String::from_str(&env, "b"), String::from_str(&env, "<d>")),
    ];
    client.batch_issue(
        &issuer,
        &contract_id,
        &String::from_str(&env, "did:issuer"),
        &vcs,
    );
    // 2 VCs * 10.0 = 20.0
    assert_eq!(
        TokenClient::new(&env, &token_addr).balance(&dest),
        20_000_000
    );
}

// --- Targeted auth tests ---
// The main test suite uses mock_all_auths_allowing_non_root_auth() which
// bypasses require_auth() checks. These tests use no mocks to confirm that auth
// guards are actually enforced and would catch regressions where a guard is
// accidentally removed.

fn setup_no_mock() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    Address,
    VcVaultContractClient<'static>,
) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);
    let factory = env.register(MockFactory, ());
    let did_uri = String::from_str(&env, "did:test");
    let contract_id = env.register(
        VcVaultContract,
        (owner.clone(), admin.clone(), did_uri, factory.clone()),
    );
    let client = VcVaultContractClient::new(&env, &contract_id);
    (env, owner, admin, issuer, factory, contract_id, client)
}

#[test]
#[should_panic]
fn test_auth_nominate_admin_requires_current_admin_signature() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup_no_mock();
    let new_admin = Address::generate(&env);
    client.nominate_admin(&new_admin);
}

#[test]
#[should_panic]
fn test_auth_deny_issuer_requires_vault_admin_signature() {
    let (_env, _owner, _admin, issuer, _factory, _contract_id, client) = setup_no_mock();
    client.deny_issuer(&issuer);
}

#[test]
#[should_panic]
fn test_auth_set_vault_did_requires_owner_signature() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup_no_mock();
    client.set_vault_did(&String::from_str(&env, "did:stellar:testnet:x"));
}

#[test]
#[should_panic]
fn test_auth_issue_requires_issuer_signature() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup_no_mock();
    client.issue(
        &String::from_str(&env, "vc-1"),
        &String::from_str(&env, "<data>"),
        &contract_id,
        &issuer,
        &String::from_str(&env, "did:issuer"),
    );
}

// --- Event coverage ---

#[test]
fn test_set_vault_admin_emits_event() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    let new_admin = Address::generate(&env);
    client.set_vault_admin(&new_admin);

    assert_eq!(env.events().all().len(), 1);
}

// --- O(1) index tests ---

#[test]
fn test_index_remove_middle_uses_swap_and_pop() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let issuer_did = String::from_str(&env, "did:issuer");
    let data = String::from_str(&env, "<data>");
    let id_a = String::from_str(&env, "vc-a");
    let id_b = String::from_str(&env, "vc-b");
    let id_c = String::from_str(&env, "vc-c");
    client.issue(&id_a, &data, &contract_id, &issuer, &issuer_did);
    client.issue(&id_b, &data, &contract_id, &issuer, &issuer_did);
    client.issue(&id_c, &data, &contract_id, &issuer, &issuer_did);
    assert_eq!(client.list_vc_ids(&0_u32, &200_u32).len(), 3);

    client.revoke(&id_b, &String::from_str(&env, "2025-01-01T00:00:00Z"));
    let remaining = client.list_vc_ids(&0_u32, &200_u32);
    assert_eq!(remaining.len(), 2);
    assert!(remaining.contains(id_a.clone()));
    assert!(remaining.contains(id_c.clone()));
    assert!(!remaining.contains(id_b));
    // The revoked VC payload survives - only the active index is freed.
    assert_eq!(client.verify_vc(&id_a), crate::types::VCStatus::Valid);
    assert_eq!(client.verify_vc(&id_c), crate::types::VCStatus::Valid);
}

#[test]
fn test_revoke_frees_index_slot_for_reissuance_under_new_id() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let issuer_did = String::from_str(&env, "did:issuer");
    let data = String::from_str(&env, "<data>");
    let id1 = String::from_str(&env, "vc-1");
    client.issue(&id1, &data, &contract_id, &issuer, &issuer_did);
    assert_eq!(client.list_vc_ids(&0_u32, &200_u32).len(), 1);
    client.revoke(&id1, &String::from_str(&env, "2025-01-01T00:00:00Z"));
    assert_eq!(client.list_vc_ids(&0_u32, &200_u32).len(), 0);
    let id2 = String::from_str(&env, "vc-2");
    client.issue(&id2, &data, &contract_id, &issuer, &issuer_did);
    assert_eq!(client.list_vc_ids(&0_u32, &200_u32).len(), 1);
}

#[test]
fn test_index_remains_consistent_after_many_issues_and_revokes() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let issuer_did = String::from_str(&env, "did:issuer");
    let data = String::from_str(&env, "<data>");
    let revoke_date = String::from_str(&env, "2025-01-01T00:00:00Z");

    let labels = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
    let mut ids: soroban_sdk::Vec<String> = soroban_sdk::Vec::new(&env);
    for label in labels.iter() {
        let id = String::from_str(&env, label);
        client.issue(&id, &data, &contract_id, &issuer, &issuer_did);
        ids.push_back(id);
    }
    assert_eq!(client.list_vc_ids(&0_u32, &200_u32).len(), 10);

    for i in (0..10).step_by(2) {
        let id = ids.get_unchecked(i);
        client.revoke(&id, &revoke_date);
    }
    let remaining = client.list_vc_ids(&0_u32, &200_u32);
    assert_eq!(remaining.len(), 5);
    for i in (1..10).step_by(2) {
        let id = ids.get_unchecked(i);
        assert!(remaining.contains(id));
    }
}

// --- Pagination tests ---

#[test]
fn test_vc_count_is_zero_for_empty_vault() {
    let (_env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    assert_eq!(client.vc_count(), 0);
}

#[test]
fn test_vc_count_tracks_issue_and_revoke() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let issuer_did = String::from_str(&env, "did:issuer");
    let data = String::from_str(&env, "<data>");

    assert_eq!(client.vc_count(), 0);
    let id_a = String::from_str(&env, "vc-a");
    let id_b = String::from_str(&env, "vc-b");
    client.issue(&id_a, &data, &contract_id, &issuer, &issuer_did);
    client.issue(&id_b, &data, &contract_id, &issuer, &issuer_did);
    assert_eq!(client.vc_count(), 2);

    client.revoke(&id_a, &String::from_str(&env, "2025-01-01T00:00:00Z"));
    assert_eq!(client.vc_count(), 1);
}

#[test]
fn test_list_vc_ids_paginates_consistently() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let issuer_did = String::from_str(&env, "did:issuer");
    let data = String::from_str(&env, "<data>");
    for label in ["a", "b", "c", "d", "e"].iter() {
        let id = String::from_str(&env, label);
        client.issue(&id, &data, &contract_id, &issuer, &issuer_did);
    }
    assert_eq!(client.vc_count(), 5);

    let all = client.list_vc_ids(&0_u32, &200_u32);
    assert_eq!(all.len(), 5);

    let first = client.list_vc_ids(&0_u32, &2_u32);
    let rest = client.list_vc_ids(&2_u32, &10_u32);
    assert_eq!(first.len(), 2);
    assert_eq!(rest.len(), 3);
    let mut joined = soroban_sdk::Vec::<String>::new(&env);
    for id in first.iter() {
        joined.push_back(id);
    }
    for id in rest.iter() {
        joined.push_back(id);
    }
    assert_eq!(joined.len(), 5);
    for id in all.iter() {
        assert!(joined.contains(id));
    }
}

#[test]
fn test_list_vc_ids_zero_limit_returns_empty() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    client.issue(
        &String::from_str(&env, "vc-1"),
        &String::from_str(&env, "<data>"),
        &contract_id,
        &issuer,
        &String::from_str(&env, "did:issuer"),
    );
    let result = client.list_vc_ids(&0_u32, &0_u32);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_list_vc_ids_offset_beyond_count_returns_empty() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    client.issue(
        &String::from_str(&env, "vc-1"),
        &String::from_str(&env, "<data>"),
        &contract_id,
        &issuer,
        &String::from_str(&env, "did:issuer"),
    );
    let result = client.list_vc_ids(&5_u32, &10_u32);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_list_vc_ids_limit_clamped_to_count() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let issuer_did = String::from_str(&env, "did:issuer");
    let data = String::from_str(&env, "<data>");
    for label in ["a", "b", "c"].iter() {
        let id = String::from_str(&env, label);
        client.issue(&id, &data, &contract_id, &issuer, &issuer_did);
    }
    let result = client.list_vc_ids(&0_u32, &200_u32);
    assert_eq!(result.len(), 3);
}

#[test]
#[should_panic(expected = "Error(Contract, #16)")] // LimitTooLarge
fn test_list_vc_ids_limit_above_max_panics() {
    let (_env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    // MAX_LIST_LIMIT = 200; 201 must panic.
    client.list_vc_ids(&0_u32, &201_u32);
}

// --- batch_issue tests ---

#[test]
fn test_batch_issue_writes_all_vcs_in_order() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let issuer_did = String::from_str(&env, "did:issuer");
    let data = String::from_str(&env, "<data>");

    let id_a = String::from_str(&env, "vc-a");
    let id_b = String::from_str(&env, "vc-b");
    let id_c = String::from_str(&env, "vc-c");
    let vcs = vec![
        &env,
        (id_a.clone(), data.clone()),
        (id_b.clone(), data.clone()),
        (id_c.clone(), data.clone()),
    ];
    let returned = client.batch_issue(&issuer, &contract_id, &issuer_did, &vcs);

    assert_eq!(returned.len(), 3);
    assert_eq!(returned.get_unchecked(0), id_a);
    assert_eq!(returned.get_unchecked(1), id_b);
    assert_eq!(returned.get_unchecked(2), id_c);
    assert_eq!(client.vc_count(), 3);
    let listed = client.list_vc_ids(&0_u32, &10_u32);
    assert!(listed.contains(id_a));
    assert!(listed.contains(id_b));
    assert!(listed.contains(id_c));
}

#[test]
fn test_batch_issue_at_max_size_succeeds() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let issuer_did = String::from_str(&env, "did:issuer");
    let data = String::from_str(&env, "<data>");

    let vcs = vec![
        &env,
        (String::from_str(&env, "vc-1"), data.clone()),
        (String::from_str(&env, "vc-2"), data.clone()),
        (String::from_str(&env, "vc-3"), data.clone()),
        (String::from_str(&env, "vc-4"), data.clone()),
        (String::from_str(&env, "vc-5"), data.clone()),
    ];
    let returned = client.batch_issue(&issuer, &contract_id, &issuer_did, &vcs);
    assert_eq!(returned.len(), 5);
    assert_eq!(client.vc_count(), 5);
}

#[test]
#[should_panic(expected = "Error(Contract, #17)")] // BatchTooLarge
fn test_batch_issue_above_max_size_panics() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let data = String::from_str(&env, "<data>");

    let vcs = vec![
        &env,
        (String::from_str(&env, "vc-1"), data.clone()),
        (String::from_str(&env, "vc-2"), data.clone()),
        (String::from_str(&env, "vc-3"), data.clone()),
        (String::from_str(&env, "vc-4"), data.clone()),
        (String::from_str(&env, "vc-5"), data.clone()),
        (String::from_str(&env, "vc-6"), data.clone()),
    ];
    client.batch_issue(
        &issuer,
        &contract_id,
        &String::from_str(&env, "did:issuer"),
        &vcs,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")] // BatchEmpty
fn test_batch_issue_empty_panics() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let vcs = soroban_sdk::Vec::<(String, String)>::new(&env);
    client.batch_issue(
        &issuer,
        &contract_id,
        &String::from_str(&env, "did:issuer"),
        &vcs,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")] // VCAlreadyExists
fn test_batch_issue_with_duplicate_within_batch_panics() {
    // First entry writes vc-x; second entry's existence check finds it and
    // panics with VCAlreadyExists.
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let data = String::from_str(&env, "<data>");
    let dup = String::from_str(&env, "vc-x");
    let vcs = vec![&env, (dup.clone(), data.clone()), (dup, data)];
    client.batch_issue(
        &issuer,
        &contract_id,
        &String::from_str(&env, "did:issuer"),
        &vcs,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")] // VCAlreadyExists
fn test_batch_issue_with_existing_vc_panics() {
    // A VC with this id was previously issued; batch's existence check
    // catches it on the first iteration.
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let issuer_did = String::from_str(&env, "did:issuer");
    let data = String::from_str(&env, "<data>");

    client.issue(
        &String::from_str(&env, "vc-existing"),
        &data,
        &contract_id,
        &issuer,
        &issuer_did,
    );

    let vcs = vec![
        &env,
        (String::from_str(&env, "vc-new"), data.clone()),
        (String::from_str(&env, "vc-existing"), data),
    ];
    client.batch_issue(&issuer, &contract_id, &issuer_did, &vcs);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")] // VaultRevoked
fn test_batch_issue_on_revoked_vault_panics() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    client.revoke_vault();
    let data = String::from_str(&env, "<data>");
    let vcs = vec![&env, (String::from_str(&env, "vc-1"), data)];
    client.batch_issue(
        &issuer,
        &contract_id,
        &String::from_str(&env, "did:issuer"),
        &vcs,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")] // InvalidVaultContract
fn test_batch_issue_with_wrong_vault_contract_panics() {
    let (env, _owner, _admin, issuer, _factory, _contract_id, client) = setup();
    let wrong_contract = Address::generate(&env);
    let data = String::from_str(&env, "<data>");
    let vcs = vec![&env, (String::from_str(&env, "vc-1"), data)];
    client.batch_issue(
        &issuer,
        &wrong_contract,
        &String::from_str(&env, "did:issuer"),
        &vcs,
    );
}

#[test]
fn test_batch_issue_emits_one_event_per_vc() {
    // Off-chain indexers expect one VCIssued per credential, even when the
    // credentials are written together. Capture events before any read so
    // env.events().all() still holds them.
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let data = String::from_str(&env, "<data>");
    let vcs = vec![
        &env,
        (String::from_str(&env, "vc-a"), data.clone()),
        (String::from_str(&env, "vc-b"), data.clone()),
        (String::from_str(&env, "vc-c"), data),
    ];
    client.batch_issue(
        &issuer,
        &contract_id,
        &String::from_str(&env, "did:issuer"),
        &vcs,
    );
    // 3 VCIssued events from the batch (no other contract calls between).
    assert_eq!(env.events().all().len(), 3);
}

// --- Input length cap tests ---

fn long_string(env: &Env, byte: u8, n: usize) -> String {
    extern crate alloc;
    let v: alloc::vec::Vec<u8> = (0..n).map(|_| byte).collect();
    String::from_bytes(env, &v)
}

#[test]
fn test_constructor_accepts_did_uri_at_max_len() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let admin = Address::generate(&env);
    let did_uri = long_string(&env, b'd', 256); // MAX_DID_URI_LEN
    env.register(
        VcVaultContract,
        (owner, admin, did_uri, Address::generate(&env)),
    );
    // No panic means success.
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")] // InputTooLong
fn test_constructor_rejects_did_uri_over_max_len() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let admin = Address::generate(&env);
    let did_uri = long_string(&env, b'd', 257);
    env.register(
        VcVaultContract,
        (owner, admin, did_uri, Address::generate(&env)),
    );
}

#[test]
fn test_issue_accepts_vc_id_at_max_len() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let vc_id = long_string(&env, b'a', 64); // MAX_VC_ID_LEN
    client.issue(
        &vc_id,
        &String::from_str(&env, "<data>"),
        &contract_id,
        &issuer,
        &String::from_str(&env, "did:issuer"),
    );
    assert_eq!(client.vc_count(), 1);
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")] // InputTooLong
fn test_issue_rejects_vc_id_over_max_len() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let vc_id = long_string(&env, b'a', 65);
    client.issue(
        &vc_id,
        &String::from_str(&env, "<data>"),
        &contract_id,
        &issuer,
        &String::from_str(&env, "did:issuer"),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")] // InputTooLong
fn test_issue_rejects_vc_data_over_max_len() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let vc_data = long_string(&env, b'd', 10_001);
    client.issue(
        &String::from_str(&env, "vc-1"),
        &vc_data,
        &contract_id,
        &issuer,
        &String::from_str(&env, "did:issuer"),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")] // InputTooLong
fn test_issue_rejects_issuer_did_over_max_len() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let issuer_did = long_string(&env, b'i', 257);
    client.issue(
        &String::from_str(&env, "vc-1"),
        &String::from_str(&env, "<data>"),
        &contract_id,
        &issuer,
        &issuer_did,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")] // InputTooLong
fn test_revoke_rejects_date_over_max_len() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let vc_id = String::from_str(&env, "vc-1");
    client.issue(
        &vc_id,
        &String::from_str(&env, "<data>"),
        &contract_id,
        &issuer,
        &String::from_str(&env, "did:issuer"),
    );
    let date = long_string(&env, b'X', 65); // MAX_DATE_LEN = 64
    client.revoke(&vc_id, &date);
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")] // InputTooLong
fn test_batch_issue_rejects_oversized_vc_id_within_batch() {
    // The cap applies inside batch_issue too: even if 4 entries are valid, a
    // 5th oversize id rejects the whole batch.
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();
    let data = String::from_str(&env, "<data>");
    let bad_id = long_string(&env, b'z', 65);
    let vcs = vec![
        &env,
        (String::from_str(&env, "vc-1"), data.clone()),
        (bad_id, data),
    ];
    client.batch_issue(
        &issuer,
        &contract_id,
        &String::from_str(&env, "did:issuer"),
        &vcs,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")] // InputTooLong
fn test_get_vc_rejects_oversized_vc_id() {
    // Read paths cap the input too so an attacker can't force the contract
    // to spend instructions hashing a 1MB key before the lookup misses.
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    let vc_id = long_string(&env, b'q', 65);
    client.get_vc(&vc_id);
}

// --- Event emission tests ---
//
// env.events().all() returns events from the last invocation only, so each
// test calls the entrypoint and asserts the event count immediately - // before any other contract call that would clear the event buffer.

#[test]
fn test_constructor_emits_contract_initialized_and_vault_created() {
    use crate::events::{ContractInitialized, VaultCreated};
    use soroban_sdk::{Event as SorobanEvent, Map, Symbol, TryFromVal, Val};
    let env = Env::default();
    let owner = Address::generate(&env);
    let admin = Address::generate(&env);
    let did_uri = String::from_str(&env, "did:test");
    env.register(
        VcVaultContract,
        (
            owner.clone(),
            admin.clone(),
            did_uri.clone(),
            Address::generate(&env),
        ),
    );
    let events = env.events().all();
    assert_eq!(events.len(), 2);
    let (_, topics0, data0) = events.get(0).unwrap();
    let expected0 = ContractInitialized {
        admin: admin.clone(),
    };
    assert_eq!(topics0, expected0.topics(&env));
    assert_eq!(
        Map::<Symbol, Val>::try_from_val(&env, &data0).unwrap(),
        Map::<Symbol, Val>::try_from_val(&env, &expected0.data(&env)).unwrap(),
    );
    let (_, topics1, data1) = events.get(1).unwrap();
    let expected1 = VaultCreated {
        owner: owner.clone(),
        did_uri: did_uri.clone(),
    };
    assert_eq!(topics1, expected1.topics(&env));
    assert_eq!(
        Map::<Symbol, Val>::try_from_val(&env, &data1).unwrap(),
        Map::<Symbol, Val>::try_from_val(&env, &expected1.data(&env)).unwrap(),
    );
}

#[test]
fn test_nominate_admin_emits_admin_nominated() {
    use crate::events::AdminNominated;
    use soroban_sdk::{Event as SorobanEvent, Map, Symbol, TryFromVal, Val};
    let (env, _owner, admin, _issuer, _factory, _contract_id, client) = setup();
    let nominee = Address::generate(&env);
    client.nominate_admin(&nominee);
    let events = env.events().all();
    assert_eq!(events.len(), 1);
    let (_, topics, data) = events.get(0).unwrap();
    let expected = AdminNominated {
        current_admin: admin.clone(),
        nominee: nominee.clone(),
    };
    assert_eq!(topics, expected.topics(&env));
    assert_eq!(
        Map::<Symbol, Val>::try_from_val(&env, &data).unwrap(),
        Map::<Symbol, Val>::try_from_val(&env, &expected.data(&env)).unwrap(),
    );
}

#[test]
fn test_accept_contract_admin_emits_admin_transferred() {
    use crate::events::AdminTransferred;
    use soroban_sdk::{Event as SorobanEvent, Map, Symbol, TryFromVal, Val};
    let (env, _owner, admin, _issuer, _factory, _contract_id, client) = setup();
    let nominee = Address::generate(&env);
    client.nominate_admin(&nominee);
    // accept_contract_admin emits one event in its own invocation; the prior
    // nominate_admin event is in a separate invocation and not in this buffer.
    client.accept_contract_admin();
    let events = env.events().all();
    assert_eq!(events.len(), 1);
    let (_, topics, data) = events.get(0).unwrap();
    let expected = AdminTransferred {
        old_admin: admin.clone(),
        new_admin: nominee.clone(),
    };
    assert_eq!(topics, expected.topics(&env));
    assert_eq!(
        Map::<Symbol, Val>::try_from_val(&env, &data).unwrap(),
        Map::<Symbol, Val>::try_from_val(&env, &expected.data(&env)).unwrap(),
    );
}

// --- Denied-issuer O(1) index tests ---

#[test]
fn test_list_denied_issuers_pagination() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    let i1 = Address::generate(&env);
    let i2 = Address::generate(&env);
    client.deny_issuer(&i1);
    client.deny_issuer(&i2);
    let all = client.list_denied_issuers(&0_u32, &100_u32);
    assert_eq!(all.len(), 2);
    let page1 = client.list_denied_issuers(&0_u32, &1_u32);
    assert_eq!(page1.len(), 1);
    let empty = client.list_denied_issuers(&5_u32, &100_u32);
    assert_eq!(empty.len(), 0);
}

#[test]
fn test_deny_issuer_updates_index() {
    let (env, _owner, _admin, _issuer, _factory, _contract_id, client) = setup();
    assert_eq!(client.denied_issuer_count(), 0);
    let iss = Address::generate(&env);
    client.deny_issuer(&iss);
    assert_eq!(client.denied_issuer_count(), 1);
    let denied = client.list_denied_issuers(&0_u32, &100_u32);
    assert!(denied.contains(iss.clone()));
    client.allow_issuer(&iss);
    assert_eq!(client.denied_issuer_count(), 0);
    let denied = client.list_denied_issuers(&0_u32, &100_u32);
    assert!(!denied.contains(iss));
}

#[test]
#[should_panic(expected = "Error(Contract, #15)")] // VaultFull
fn test_vault_full_at_u32_max() {
    let (env, _owner, _admin, issuer, _factory, contract_id, client) = setup();

    // Seed VaultVCCount = u32::MAX directly to simulate overflow boundary.
    env.as_contract(&contract_id, || {
        let key = crate::storage::VcVaultDataKey::VaultVCCount;
        env.storage().persistent().set(&key, &u32::MAX);
    });

    client.issue(
        &String::from_str(&env, "overflow-vc"),
        &String::from_str(&env, "data"),
        &contract_id,
        &issuer,
        &String::from_str(&env, "did:issuer"),
    );
}
