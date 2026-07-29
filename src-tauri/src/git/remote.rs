#[cfg(test)]
use std::path::Path;

use git2::{
    BranchType, Cred, CredentialType, FetchOptions, PushOptions, Remote, RemoteCallbacks,
    Repository,
};

use super::repository::current_branch;
use crate::{
    error::{AppError, AppResult},
    types::CredentialRecord,
};

pub(super) enum UpstreamDisposition {
    Configured,
    MissingRemoteBranch,
}

pub(super) fn fetch_options(credential: Option<CredentialRecord>) -> FetchOptions<'static> {
    let mut options = FetchOptions::new();
    options.remote_callbacks(remote_callbacks(credential));
    options
}

pub(super) fn fetch_remote(
    repo: &Repository,
    credential: Option<CredentialRecord>,
) -> AppResult<()> {
    let (_, mut remote) = selected_remote(repo)?;
    let mut options = fetch_options(credential);
    remote.fetch(&[] as &[&str], Some(&mut options), None)?;
    Ok(())
}

pub(super) fn push_current_branch(
    repo: &Repository,
    credential: Option<CredentialRecord>,
) -> AppResult<()> {
    let branch = current_branch(repo).ok_or_else(|| {
        AppError::Message("A named branch is required before pushing.".to_owned())
    })?;
    let (remote_name, mut remote) = selected_remote(repo)?;
    let callbacks = remote_callbacks(credential);
    let mut options = PushOptions::new();
    options.remote_callbacks(callbacks);
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    remote.push(&[&refspec], Some(&mut options))?;

    if let Ok(mut local_branch) = repo.find_branch(&branch, BranchType::Local) {
        if local_branch.upstream().is_err() {
            let _ = local_branch.set_upstream(Some(&format!("{remote_name}/{branch}")));
        }
    }
    Ok(())
}

pub(super) fn ensure_current_branch_upstream(repo: &Repository) -> AppResult<UpstreamDisposition> {
    let branch_name = current_branch(repo).ok_or_else(|| {
        AppError::Message("A named branch is required for this Git operation.".to_owned())
    })?;
    let mut branch = repo.find_branch(&branch_name, BranchType::Local)?;
    if branch.upstream().is_ok() {
        return Ok(UpstreamDisposition::Configured);
    }
    let remote_name = selected_remote_name(repo)?;
    let remote_reference = format!("refs/remotes/{remote_name}/{branch_name}");
    if repo.find_reference(&remote_reference).is_err() {
        return Ok(UpstreamDisposition::MissingRemoteBranch);
    }
    branch.set_upstream(Some(&format!("{remote_name}/{branch_name}")))?;
    Ok(UpstreamDisposition::Configured)
}

pub(super) fn remote_url(repo: &Repository) -> Option<String> {
    selected_remote_name(repo)
        .ok()
        .and_then(|name| repo.find_remote(&name).ok())
        .and_then(|remote| remote.url().ok().map(str::to_owned))
}

pub(super) fn validate_remote_url(remote_url: &str) -> AppResult<()> {
    if url::Url::parse(remote_url)
        .ok()
        .is_some_and(|url| url.scheme() == "https" && url.host_str().is_some())
    {
        return Ok(());
    }
    #[cfg(test)]
    if Path::new(remote_url).exists()
        || url::Url::parse(remote_url)
            .ok()
            .is_some_and(|url| url.scheme() == "file")
    {
        return Ok(());
    }
    Err(AppError::Message(
        "Marktree Git remotes must use an HTTPS URL.".to_owned(),
    ))
}

fn selected_remote(repo: &Repository) -> AppResult<(String, Remote<'_>)> {
    let remote_name = selected_remote_name(repo)?;
    let remote = repo
        .find_remote(&remote_name)
        .map_err(|_| AppError::Message("No configured Git remote was found.".to_owned()))?;
    let url = remote
        .url()
        .map_err(|_| AppError::Message("The Git remote URL is not valid UTF-8.".to_owned()))?;
    if url.is_empty() {
        return Err(AppError::Message("The Git remote has no URL.".to_owned()));
    }
    validate_remote_url(url)?;
    Ok((remote_name, remote))
}

fn selected_remote_name(repo: &Repository) -> AppResult<String> {
    if let Some(name) = upstream_remote_name(repo) {
        return Ok(name);
    }
    if repo.find_remote("origin").is_ok() {
        return Ok("origin".to_owned());
    }
    let remotes = repo.remotes()?;
    let names = remotes
        .iter()
        .filter_map(Result::ok)
        .flatten()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    match names.as_slice() {
        [name] => Ok(name.clone()),
        [] => Err(AppError::Message(
            "No configured Git remote was found.".to_owned(),
        )),
        _ => Err(AppError::Message(
            "Multiple Git remotes exist and the current branch has no upstream.".to_owned(),
        )),
    }
}

fn upstream_remote_name(repo: &Repository) -> Option<String> {
    let branch = current_branch(repo)?;
    repo.branch_upstream_remote(&format!("refs/heads/{branch}"))
        .ok()
        .and_then(|buffer| buffer.as_str().ok().map(str::to_owned))
}

fn remote_callbacks(credential: Option<CredentialRecord>) -> RemoteCallbacks<'static> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, username_from_url, allowed| {
        if let Some(record) = credential.as_ref() {
            if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
                let username = if record.username.trim().is_empty() {
                    username_from_url.unwrap_or("x-access-token")
                } else {
                    record.username.as_str()
                };
                return Cred::userpass_plaintext(username, &record.token);
            }
        }
        Cred::default()
    });
    callbacks
}
