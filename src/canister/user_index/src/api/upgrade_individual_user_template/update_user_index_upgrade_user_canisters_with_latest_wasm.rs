use std::time::SystemTime;

use candid::Principal;
use ic_cdk::api::management_canister::{
    main::{self, CanisterInstallMode},
    provisional::CanisterIdRecord,
};

use shared_utils::{
    canister_specific::individual_user_template::types::arg::IndividualUserTemplateInitArgs,
    common::utils::{system_time, task},
};

use crate::{
    data_model::{configuration::Configuration, CanisterData},
    util::canister_management::{self, recharge_canister_if_below_threshold},
    CANISTER_DATA,
};

const MAX_CONCURRENCY: usize = 11;

pub async fn upgrade_user_canisters_with_latest_wasm() {
    let mut upgrade_count = 0;
    let mut failed_canister_ids = Vec::new();

    let user_principal_id_to_canister_id_map = CANISTER_DATA.with(|canister_data_ref_cell| {
        canister_data_ref_cell
            .borrow()
            .user_principal_id_to_canister_id_map
            .clone()
    });

    let saved_upgrade_status = CANISTER_DATA.with(|canister_data_ref_cell| {
        canister_data_ref_cell
            .borrow()
            .last_run_upgrade_status
            .clone()
    });

    let configuration = CANISTER_DATA
        .with(|canister_data_ref_cell| canister_data_ref_cell.borrow().configuration.clone());

   let upgrade_individual_canister_futures = user_principal_id_to_canister_id_map.iter()
    .map(|(user_principal_id, user_canister_id)| {
        recharge_and_upgrade(*user_canister_id, *user_principal_id, saved_upgrade_status.version_number, configuration.clone(), saved_upgrade_status.version.clone())
    });

    let result_callback = |upgrade_result: Result<Principal, (Principal, String)>| {

        if upgrade_result.is_err() {
            let (done_user_principal_id, err) = upgrade_result.err().unwrap();
            let done_user_canister_id = user_principal_id_to_canister_id_map.get(&done_user_principal_id).unwrap();
            ic_cdk::print(format!(
                "Failed to upgrade canister: {:?} with error: {:?}",
                done_user_canister_id.to_text(),
                err
            ));
            failed_canister_ids.push((done_user_principal_id, *done_user_canister_id, err));
        }
        
        upgrade_count += 1;
        CANISTER_DATA.with(|canister_data_ref_cell| {
            update_upgrade_status(
                &mut canister_data_ref_cell.borrow_mut(),
                upgrade_count,
                &failed_canister_ids,
                None,
                None,
            );
        });
    };

    let breaking_condition = || {
        !CANISTER_DATA.with(|canister_data_ref| canister_data_ref.borrow().allow_upgrades_for_individual_canisters)
    };

    task::run_task_concurrently(upgrade_individual_canister_futures, MAX_CONCURRENCY, result_callback, breaking_condition).await;

    CANISTER_DATA.with(|canister_data_ref_cell| {
        update_upgrade_status(
            &mut canister_data_ref_cell.borrow_mut(),
            upgrade_count,
            &failed_canister_ids,
            Some(saved_upgrade_status.version_number + 1),
            Some(system_time::get_current_system_time_from_ic()),
        );
    });
}


async fn recharge_and_upgrade(user_canister_id: Principal, user_principal_id: Principal, version_number: u64, configuration: Configuration, version: String) -> Result<Principal, (Principal, String)> {
    recharge_canister_if_below_threshold(&user_canister_id).await?;
    
    upgrade_user_canister(&user_principal_id, &user_canister_id, version_number, &configuration, version).await.map_err(|s| (user_principal_id, s))?;

    Ok(user_principal_id)
}

async fn upgrade_user_canister(
    user_principal_id: &Principal,
    canister_id: &Principal,
    version_number: u64,
    configuration: &Configuration,
    version: String
) -> Result<(), String> {
    canister_management::upgrade_individual_user_canister(
        *canister_id,
        CanisterInstallMode::Upgrade,
        IndividualUserTemplateInitArgs {
            known_principal_ids: Some(configuration.known_principal_ids.clone()),
            profile_owner: Some(*user_principal_id),
            upgrade_version_number: Some(version_number + 1),
            url_to_send_canister_metrics_to: Some(
                configuration.url_to_send_canister_metrics_to.clone(),
            ),
            version
        },
    )
    .await
    .map_err(|e| e.1)
}

fn update_upgrade_status(
    canister_data: &mut CanisterData,
    upgrade_count: u32,
    failed_canister_ids: &[(Principal, Principal, String)],
    version_number: Option<u64>,
    last_run_on: Option<SystemTime>,
) {
    let mut last_run_upgrade_status = canister_data.last_run_upgrade_status.clone();

    last_run_upgrade_status.successful_upgrade_count = upgrade_count;
    last_run_upgrade_status.failed_canister_ids = failed_canister_ids.to_owned();
    last_run_upgrade_status.version_number =
        version_number.unwrap_or(canister_data.last_run_upgrade_status.version_number);
    last_run_upgrade_status.last_run_on =
        last_run_on.unwrap_or(canister_data.last_run_upgrade_status.last_run_on);

    canister_data.last_run_upgrade_status = last_run_upgrade_status;
}
