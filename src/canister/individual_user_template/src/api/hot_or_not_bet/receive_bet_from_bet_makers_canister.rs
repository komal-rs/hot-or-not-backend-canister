use std::time::SystemTime;

use candid::Principal;
use ic_cdk::api::management_canister::provisional::CanisterId;
use shared_utils::{
    canister_specific::individual_user_template::types::{
        arg::PlaceBetArg,
        error::BetOnCurrentlyViewingPostError,
        hot_or_not::{BetDirection, BettingStatus},
    },
    common::utils::system_time,
};

use crate::{
    api::post::update_scores_and_share_with_post_cache_if_difference_beyond_threshold::update_scores_and_share_with_post_cache_if_difference_beyond_threshold,
    data_model::CanisterData, CANISTER_DATA,
};

#[ic_cdk::update]
#[candid::candid_method(update)]
fn receive_bet_from_bet_makers_canister(
    place_bet_arg: PlaceBetArg,
    bet_maker_principal_id: Principal,
) -> Result<BettingStatus, BetOnCurrentlyViewingPostError> {
    let bet_maker_canister_id = ic_cdk::caller();

    let status = CANISTER_DATA.with(|canister_data_ref_cell| {
        receive_bet_from_bet_makers_canister_impl(
            &mut canister_data_ref_cell.borrow_mut(),
            &bet_maker_principal_id,
            &bet_maker_canister_id,
            place_bet_arg.clone(),
            &system_time::get_current_system_time_from_ic(),
        )
    })?;

    CANISTER_DATA.with(|canister_data_ref_cell| {
        update_profile_stats_with_bet_placed(
            &mut canister_data_ref_cell.borrow_mut(),
            &place_bet_arg.bet_direction,
        );
    });

    update_scores_and_share_with_post_cache_if_difference_beyond_threshold(&place_bet_arg.post_id);

    Ok(status)
}

fn receive_bet_from_bet_makers_canister_impl(
    canister_data: &mut CanisterData,
    bet_maker_principal_id: &Principal,
    bet_maker_canister_id: &CanisterId,
    place_bet_arg: PlaceBetArg,
    current_time: &SystemTime,
) -> Result<BettingStatus, BetOnCurrentlyViewingPostError> {
    let PlaceBetArg {
        post_id,
        bet_amount,
        bet_direction,
        ..
    } = place_bet_arg;

    let post = canister_data.all_created_posts.get_mut(&post_id).unwrap();

    post.place_hot_or_not_bet(
        bet_maker_principal_id,
        bet_maker_canister_id,
        bet_amount,
        &bet_direction,
        current_time,
    )
}

fn update_profile_stats_with_bet_placed(
    canister_data: &mut CanisterData,
    bet_direction: &BetDirection,
) {
    match *bet_direction {
        BetDirection::Hot => {
            canister_data.profile.profile_stats.hot_bets_received += 1;
        }
        BetDirection::Not => {
            canister_data.profile.profile_stats.not_bets_received += 1;
        }
    }
}

#[cfg(test)]
mod test {
    use shared_utils::canister_specific::individual_user_template::types::{
        hot_or_not::BetDirection,
        post::{Post, PostDetailsFromFrontend},
    };
    use test_utils::setup::test_constants::{
        get_mock_user_alice_canister_id, get_mock_user_alice_principal_id,
    };

    use super::*;

    #[test]
    fn test_receive_bet_from_bet_makers_canister_impl() {
        let mut canister_data = CanisterData::default();
        canister_data.all_created_posts.insert(
            0,
            Post::new(
                0,
                &PostDetailsFromFrontend {
                    is_nsfw: false,
                    description: "Doggos and puppers".into(),
                    hashtags: vec!["doggo".into(), "pupper".into()],
                    video_uid: "abcd#1234".into(),
                    creator_consent_for_inclusion_in_hot_or_not: true,
                },
                &SystemTime::now(),
            ),
        );

        let result = receive_bet_from_bet_makers_canister_impl(
            &mut canister_data,
            &get_mock_user_alice_principal_id(),
            &get_mock_user_alice_canister_id(),
            PlaceBetArg {
                post_canister_id: get_mock_user_alice_canister_id(),
                post_id: 0,
                bet_amount: 100,
                bet_direction: BetDirection::Hot,
            },
            &SystemTime::now(),
        );

        let post = canister_data.all_created_posts.get(&0).unwrap();

        assert_eq!(
            result,
            Ok(BettingStatus::BettingOpen {
                started_at: post.created_at,
                number_of_participants: 1,
                ongoing_slot: 1,
                ongoing_room: 1,
                has_this_user_participated_in_this_post: Some(true)
            })
        );
    }
}
