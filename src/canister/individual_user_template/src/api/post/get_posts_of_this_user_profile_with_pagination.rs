use shared_utils::{
    canister_specific::individual_user_template::types::{
        error::GetPostsOfUserProfileError, post::PostDetailsForFrontend,
        profile::UserProfileDetailsForFrontend,
    },
    common::utils::system_time,
    pagination::{self, PaginationError},
};

use crate::CANISTER_DATA;

#[ic_cdk::query]
#[candid::candid_method(query)]
fn get_posts_of_this_user_profile_with_pagination(
    from_inclusive_id: u64,
    to_exclusive_id: u64,
) -> Result<Vec<PostDetailsForFrontend>, GetPostsOfUserProfileError> {
    let (from_inclusive_id, to_exclusive_id) = pagination::get_pagination_bounds(
        from_inclusive_id,
        to_exclusive_id,
        CANISTER_DATA
            .with(|canister_data_ref_cell| canister_data_ref_cell.borrow().all_created_posts.len())
            as u64,
    )
    .map_err(|e| match e {
        PaginationError::InvalidBoundsPassed => GetPostsOfUserProfileError::InvalidBoundsPassed,
        PaginationError::ReachedEndOfItemsList => GetPostsOfUserProfileError::ReachedEndOfItemsList,
        PaginationError::ExceededMaxNumberOfItemsAllowedInOneRequest => {
            GetPostsOfUserProfileError::ExceededMaxNumberOfItemsAllowedInOneRequest
        }
    })?;

    let api_caller = ic_cdk::caller();

    Ok((from_inclusive_id..to_exclusive_id)
        .map(|id| {
            CANISTER_DATA.with(|canister_data_ref_cell| {
                let post = canister_data_ref_cell
                    .borrow()
                    .all_created_posts
                    .get(&id)
                    .unwrap()
                    .clone();
                let profile = &canister_data_ref_cell.borrow().profile;
                let followers = &canister_data_ref_cell.borrow().principals_that_follow_me;
                let following = &canister_data_ref_cell.borrow().principals_i_follow;

                post.get_post_details_for_frontend_for_this_post(
                    UserProfileDetailsForFrontend {
                        display_name: profile.display_name.clone(),
                        followers_count: followers.len() as u64,
                        following_count: following.len() as u64,
                        principal_id: profile.principal_id.unwrap(),
                        profile_picture_url: profile.profile_picture_url.clone(),
                        profile_stats: profile.profile_stats.clone(),
                        unique_user_name: profile.unique_user_name.clone(),
                    },
                    api_caller,
                    &system_time::get_current_system_time_from_ic,
                )
            })
        })
        .collect())
}
