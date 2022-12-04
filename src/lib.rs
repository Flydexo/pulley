//! pulley
#[macro_use]
extern crate pbc_contract_codegen;

use std::vec;
use pbc_contract_common::address::Address;
use pbc_contract_common::context::ContractContext;
use pbc_contract_common::events::EventGroup;
use pbc_contract_common::zk::{ZkState, ZkInputDef, ZkStateChange, CalculationStatus, SecretVarId, AttestationId};
use pbc_traits::ReadWriteRPC;
use read_write_state_derive::ReadWriteState;

#[derive(ReadWriteState, Clone)]
struct SecretVarMetadata {
}

#[state]
struct SurveyState {
    owner: Address,
    end_time: i64,
    results: Vec<u128>
}

/// Initialize the contract
/// 
/// ### Arguments
/// - `end_time`: UTC timestamp when the survey will be closed
#[init]
fn initialize(
    ctx: ContractContext,
    _zk_state: ZkState<SecretVarMetadata>,
    end_time: i64
) -> SurveyState {

    assert!(end_time > ctx.block_production_time, "Invalid time");

    let state = SurveyState {
        owner: ctx.sender,
        end_time,
        results: vec![]
    };

    state
}

#[zk_on_secret_input(shortname=0x01)]
fn vote(
    ctx: ContractContext,
    state: SurveyState,
    zk_state: ZkState<SecretVarMetadata>
) -> (SurveyState, Vec<EventGroup>, ZkInputDef<SecretVarMetadata>) {
    assert!(state.end_time > ctx.block_production_time, "Survey Ended");
    assert!(
        zk_state.secret_variables
        .iter()
        .chain(zk_state.pending_inputs.iter())
        .all(|x| x.owner != ctx.sender),
        "Cannot vote more than once"
    );
    let def = ZkInputDef {
        seal: false, 
        expected_bit_lengths: vec![8, 1],
        metadata: SecretVarMetadata{
        }
    };
    (state, vec![], def)
}

#[action(shortname=0x01)]
fn compute_results(
    ctx: ContractContext,
    state: SurveyState,
    zk_state: ZkState<SecretVarMetadata>
) -> (SurveyState, Vec<EventGroup>, Vec<ZkStateChange>) {
    assert!(ctx.block_production_time > state.end_time, "Survey not ended");
    assert_eq!(
        zk_state.calculation_state,
        CalculationStatus::Waiting,
        "Computation must start from Waiting state, but was {:?}",
        zk_state.calculation_state,
    );
    assert_eq!(
        zk_state.data_attestations.len(),
        0,
        "Auction must have exactly zero data_attestations at this point"
    );
    let invariants = vec![SecretVarMetadata{};500];

    (state, vec![], vec![ZkStateChange::start_computation(invariants)])
}

/// Automatically called when the computation is completed
///
/// The only thing we do is instantly open/declassify the output variables.
#[zk_on_compute_complete]
fn survey_compute_complete(
    _context: ContractContext,
    state: SurveyState,
    zk_state: ZkState<SecretVarMetadata>,
    output_variables: Vec<SecretVarId>,
) -> (SurveyState, Vec<EventGroup>, Vec<ZkStateChange>) {
    assert_eq!(
        zk_state.data_attestations.len(),
        0,
        "Auction must have exactly zero data_attestations at this point"
    );
    (
        state,
        vec![],
        vec![ZkStateChange::OpenVariables {
            variables: output_variables,
        }],
    )
}

/// Automatically called when the auction result is declassified. Updates state to contain result,
/// and requests attestation from nodes.
#[zk_on_variables_opened]
fn open_survey_variable(
    _context: ContractContext,
    state: SurveyState,
    zk_state: ZkState<SecretVarMetadata>,
    opened_variables: Vec<SecretVarId>,
) -> (SurveyState, Vec<EventGroup>, Vec<ZkStateChange>) {
    assert_eq!(
        zk_state.data_attestations.len(),
        0,
        "Auction must have exactly zero data_attestations at this point"
    );

    let mut survey_result:Vec<u128> = vec![];

    for amount in opened_variables {
        survey_result.push(read_variable(&zk_state, Some(&amount)))
    }

    let attest_request = ZkStateChange::Attest {
        data_to_attest: serialize_as_big_endian(&survey_result),
    };

    (state, vec![], vec![attest_request])
}

/// Automatically called when some data is attested
#[zk_on_attestation_complete]
fn survey_results_attested(
    _context: ContractContext,
    mut state: SurveyState,
    zk_state: ZkState<SecretVarMetadata>,
    attestation_id: AttestationId,
) -> (SurveyState, Vec<EventGroup>, Vec<ZkStateChange>) {
    assert_eq!(
        zk_state.data_attestations.len(),
        1,
        "Survey must have exactly one attestation"
    );
    let attestation = zk_state.get_attestation(attestation_id).unwrap();

    assert_eq!(attestation.signatures.len(), 4, "Must have four signatures");

    state.results = Vec::<u128>::rpc_read_from(&mut attestation.data.as_slice());

    (state, vec![], vec![ZkStateChange::ContractDone])
}

/// Writes some value as RPC data.
fn serialize_as_big_endian<T: ReadWriteRPC>(it: &T) -> Vec<u8> {
    let mut output: Vec<u8> = vec![];
    it.rpc_write_to(&mut output).expect("Could not serialize");
    output
}

/// Reads a variable's data as some state value
fn read_variable<T: pbc_traits::ReadWriteState>(
    zk_state: &ZkState<SecretVarMetadata>,
    variable_id: Option<&SecretVarId>,
) -> T {
    let variable_id = *variable_id.unwrap();
    let variable = zk_state.get_variable(variable_id).unwrap();
    let buffer: Vec<u8> = variable.data.clone().unwrap();
    T::state_read_from(&mut buffer.as_slice())
}