//! End-to-end API behaviour: collaboration, invitations, per-owner project
//! names, key rotation, and conflicting edits from two devices.

mod harness;

use axum::http::StatusCode;
use harness::{wrapped_for, Harness};
use serde_json::{json, Value};
use sqlx::PgPool;

const CIPHERTEXT_V1: &str = "greenbyte:v3:opaque-under-key-1";
const CIPHERTEXT_V2: &str = "greenbyte:v3:opaque-under-key-2";
const ALICE_EDIT: &str = "greenbyte:v3:alice-edit";
const BOB_EDIT: &str = "greenbyte:v3:bob-edit";

#[sqlx::test]
async fn invited_collaborator_receives_a_sealed_project_key(pool: PgPool) {
    let api = Harness::new(pool).await;
    let alice = api.account("alice").await;
    let bob = api.account("bob").await;

    let (status, project) = api
        .call(
            "POST",
            "/projects",
            Some(&alice.token),
            json!({ "name": "payments", "wrapped_key": wrapped_for("alice", 1) }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let project_id = project["project_id"].as_str().unwrap().to_string();
    assert_eq!(project["key_version"], 1);

    // Alice can read the key sealed to her at creation.
    let (status, grants) = api
        .call(
            "GET",
            &format!("/projects/{project_id}/keys"),
            Some(&alice.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(grants[0]["key_version"], 1);
    assert_eq!(grants[0]["wrapped_key"], wrapped_for("alice", 1));

    let push = json!({
        "project_id": project_id,
        "filename": ".env",
        "content": CIPHERTEXT_V1,
        "message": "initial",
        "commit_id": "11111111-1111-4111-8111-111111111111",
        "key_version": 1,
    });
    let (status, pushed) = api
        .call("POST", "/users/me/env", Some(&alice.token), push.clone())
        .await;
    assert_eq!(status, StatusCode::OK);

    // A retried push with the same commit id is idempotent, not a duplicate.
    let (status, retried) = api
        .call("POST", "/users/me/env", Some(&alice.token), push)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retried["commit_id"], pushed["commit_id"]);

    // Bob is a stranger until invited.
    let (status, _) = api
        .call(
            "GET",
            &format!("/users/me/env?project_id={project_id}"),
            Some(&bob.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Alice looks up Bob's identity key, then invites him with the project key
    // sealed to it. No key ever travels out of band.
    let (status, lookup) = api
        .call(
            "POST",
            &format!("/projects/{project_id}/collaborators/lookup"),
            Some(&alice.token),
            json!({ "email": bob.email }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(lookup["public_key"], bob.public_key);

    let (status, _) = api
        .call(
            "POST",
            &format!("/projects/{project_id}/collaborators"),
            Some(&alice.token),
            json!({ "email": bob.email, "wrapped_key": wrapped_for("bob", 1) }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let ott = api.email.invitation.lock().await.clone().unwrap();

    let join = json!({
        "ott": ott,
        "refresher_token": "b".repeat(64),
        "mac_address": "aabbccddeeff",
    });

    // An invitation proves you were invited, not who you are: it is not a
    // credential, and it is bound to the account it names.
    let (status, _) = api.call("POST", "/projects/join", None, join.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = api
        .call("POST", "/projects/join", Some(&alice.token), join.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, joined) = api
        .call("POST", "/projects/join", Some(&bob.token), join)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(joined["project_name"], "payments");
    assert_eq!(joined["owner_username"], "alice");
    assert!(joined["auth_token"].is_null());
    assert!(joined["refresh_token"].is_null());

    // Bob now holds a key sealed to him, and can read the ciphertext.
    let (status, grants) = api
        .call(
            "GET",
            &format!("/projects/{project_id}/keys"),
            Some(&bob.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(grants[0]["wrapped_key"], wrapped_for("bob", 1));

    let (status, files) = api
        .call(
            "GET",
            &format!("/users/me/env?project_id={project_id}"),
            Some(&bob.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(files[0]["filename"], ".env");
    assert_eq!(files[0]["content"], CIPHERTEXT_V1);
    assert_eq!(files[0]["key_version"], 1);
}

#[sqlx::test]
async fn two_owners_may_use_the_same_project_name(pool: PgPool) {
    let api = Harness::new(pool).await;
    let alice = api.account("alice").await;
    let bob = api.account("bob").await;

    let create = |token: String| {
        let router = &api;
        async move {
            router
                .call(
                    "POST",
                    "/projects",
                    Some(&token),
                    json!({ "name": "backend", "wrapped_key": wrapped_for("owner", 1) }),
                )
                .await
        }
    };

    let (status, _) = create(alice.token.clone()).await;
    assert_eq!(status, StatusCode::OK);

    // The same name under a different owner is fine - names are scoped to the
    // account, not to the installation.
    let (status, _) = create(bob.token.clone()).await;
    assert_eq!(status, StatusCode::OK);

    // The same name twice under one owner is still a conflict, with a message
    // that explains the scoping.
    let (status, conflict) = create(alice.token).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        conflict["error"]
            .as_str()
            .unwrap()
            .contains("within your own account"),
        "{conflict}"
    );

    let (status, projects) = api
        .call("GET", "/projects", Some(&bob.token), Value::Null)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(projects[0]["name"], "backend");
    assert_eq!(projects[0]["owner_username"], "bob");
}

#[sqlx::test]
async fn rotation_retires_a_removed_member(pool: PgPool) {
    let api = Harness::new(pool).await;
    let alice = api.account("alice").await;
    let bob = api.account("bob").await;
    let project_id = collaborative_project(&api, &alice, &bob).await;

    // Bob leaves. His grants go immediately, but he may have kept the key he
    // already unwrapped - the response says so.
    let (status, removed) = api
        .call(
            "DELETE",
            &format!("/projects/{project_id}/collaborators/{}", bob.user_id),
            Some(&alice.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(removed["rotation_required"], true);

    let (status, _) = api
        .call(
            "GET",
            &format!("/projects/{project_id}/keys"),
            Some(&bob.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Alice rotates: a new key for the members who remain, every file
    // re-encrypted under it.
    let (status, rotated) = api
        .call(
            "POST",
            &format!("/projects/{project_id}/key-rotations"),
            Some(&alice.token),
            json!({
                "grants": [
                    { "user_id": alice.user_id, "wrapped_key": wrapped_for("alice", 2) }
                ],
                "files": [{ "filename": ".env", "content": CIPHERTEXT_V2 }],
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    assert_eq!(rotated["key_version"], 2);
    assert_eq!(rotated["members_granted"], 1);
    assert_eq!(rotated["files_reencrypted"], 1);

    // Alice holds both versions, so project history stays readable to her.
    let (status, grants) = api
        .call(
            "GET",
            &format!("/projects/{project_id}/keys"),
            Some(&alice.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let versions: Vec<i64> = grants
        .as_array()
        .unwrap()
        .iter()
        .map(|grant| grant["key_version"].as_i64().unwrap())
        .collect();
    assert_eq!(versions, vec![1, 2]);

    // The stored file now sits on the new key.
    let (status, files) = api
        .call(
            "GET",
            &format!("/users/me/env?project_id={project_id}"),
            Some(&alice.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(files[0]["content"], CIPHERTEXT_V2);
    assert_eq!(files[0]["key_version"], 2);

    // A client still holding the retired key cannot overwrite the file.
    let (status, stale) = api
        .call(
            "POST",
            "/users/me/env",
            Some(&alice.token),
            json!({
                "project_id": project_id,
                "filename": ".env",
                "content": CIPHERTEXT_V1,
                "message": "stale client",
                "key_version": 1,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        stale["error"].as_str().unwrap().contains("version 2"),
        "{stale}"
    );

    let (status, audit) = api
        .call(
            "GET",
            &format!("/projects/{project_id}/audit"),
            Some(&alice.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let actions: Vec<&str> = audit
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["action"].as_str().unwrap())
        .collect();
    assert!(actions.contains(&"project.key_rotated"), "{actions:?}");
    assert!(actions.contains(&"collaborator.removed"), "{actions:?}");
}

#[sqlx::test]
async fn a_partial_rotation_is_refused(pool: PgPool) {
    let api = Harness::new(pool).await;
    let alice = api.account("alice").await;
    let bob = api.account("bob").await;
    let project_id = collaborative_project(&api, &alice, &bob).await;

    // Leaving a member out would lock them out of their own project.
    let (status, missing_member) = api
        .call(
            "POST",
            &format!("/projects/{project_id}/key-rotations"),
            Some(&alice.token),
            json!({
                "grants": [
                    { "user_id": alice.user_id, "wrapped_key": wrapped_for("alice", 2) }
                ],
                "files": [{ "filename": ".env", "content": CIPHERTEXT_V2 }],
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        missing_member["error"]
            .as_str()
            .unwrap()
            .contains("all 2 current members"),
        "{missing_member}"
    );

    // Leaving a file out would leave it readable with the retired key.
    let (status, missing_file) = api
        .call(
            "POST",
            &format!("/projects/{project_id}/key-rotations"),
            Some(&alice.token),
            json!({
                "grants": [
                    { "user_id": alice.user_id, "wrapped_key": wrapped_for("alice", 2) },
                    { "user_id": bob.user_id, "wrapped_key": wrapped_for("bob", 2) }
                ],
                "files": [],
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        missing_file["error"]
            .as_str()
            .unwrap()
            .contains("all 1 stored files"),
        "{missing_file}"
    );

    // A non-admin cannot rotate at all.
    let (status, _) = api
        .call(
            "POST",
            &format!("/projects/{project_id}/key-rotations"),
            Some(&bob.token),
            json!({ "grants": [], "files": [] }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Nothing was applied: the project is still on version 1.
    let (status, files) = api
        .call(
            "GET",
            &format!("/users/me/env?project_id={project_id}"),
            Some(&alice.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(files[0]["key_version"], 1);
}

#[sqlx::test]
async fn an_account_without_an_identity_key_cannot_be_invited(pool: PgPool) {
    let api = Harness::new(pool).await;
    let alice = api.account("alice").await;

    // Register Carol but never publish a key for her.
    let (status, _) = api
        .call(
            "POST",
            "/auth/signup",
            None,
            json!({
                "username": "carol",
                "email": "carol@example.com",
                "password": "a-sufficiently-long-password",
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let verification = api
        .email
        .verifications
        .lock()
        .await
        .get("carol@example.com")
        .cloned()
        .unwrap();
    api.call(
        "POST",
        "/auth/verify-email",
        None,
        json!({ "email": "carol@example.com", "token": verification }),
    )
    .await;

    let (status, project) = api
        .call(
            "POST",
            "/projects",
            Some(&alice.token),
            json!({ "name": "payments", "wrapped_key": wrapped_for("alice", 1) }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let project_id = project["project_id"].as_str().unwrap();

    let (status, body) = api
        .call(
            "POST",
            &format!("/projects/{project_id}/collaborators/lookup"),
            Some(&alice.token),
            json!({ "email": "carol@example.com" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("has not published an identity key"),
        "{body}"
    );
}

#[sqlx::test]
async fn a_push_edited_from_a_superseded_version_is_refused(pool: PgPool) {
    let api = Harness::new(pool).await;
    let alice = api.account("alice").await;
    let bob = api.account("bob").await;
    let project_id = collaborative_project(&api, &alice, &bob).await;
    let (_, base) = current_file(&api, &alice, &project_id).await;

    // Both start from the same version. Bob pushes first.
    let (status, bobs) = api
        .call(
            "POST",
            "/users/me/env",
            Some(&bob.token),
            edit(&project_id, BOB_EDIT, Some(&base)),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{bobs}");

    // Alice's edit would silently discard Bob's, so it is refused, and the
    // response names the commit she has to merge with.
    let (status, refused) = api
        .call(
            "POST",
            "/users/me/env",
            Some(&alice.token),
            edit(&project_id, ALICE_EDIT, Some(&base)),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(refused["code"], "stale_base");
    assert_eq!(refused["current_commit_id"], bobs["commit_id"]);
    let (content, current) = current_file(&api, &alice, &project_id).await;
    assert_eq!(content, BOB_EDIT, "a refused push must write nothing");

    // She can fetch the version both started from, to merge against.
    let (status, original) = api
        .call(
            "GET",
            &format!("/projects/{project_id}/commits/{base}"),
            Some(&alice.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(original["content"], CIPHERTEXT_V1);
    assert_eq!(original["commit_id"], base.as_str());

    // Once merged on top of Bob's commit, her push goes through.
    let (status, merged) = api
        .call(
            "POST",
            "/users/me/env",
            Some(&alice.token),
            edit(&project_id, ALICE_EDIT, Some(&current)),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{merged}");

    // A client that thinks the file is new may not overwrite it either.
    let (status, refused) = api
        .call(
            "POST",
            "/users/me/env",
            Some(&bob.token),
            edit(&project_id, BOB_EDIT, None),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(refused["current_commit_id"], merged["commit_id"]);

    // Clients older than the check send no base at all, and keep
    // last-write-wins until they upgrade.
    let mut legacy = edit(&project_id, BOB_EDIT, None);
    legacy.as_object_mut().unwrap().remove("base_commit_id");
    let (status, _) = api
        .call("POST", "/users/me/env", Some(&bob.token), legacy)
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test]
async fn simultaneous_pushes_from_one_version_let_exactly_one_through(pool: PgPool) {
    let api = Harness::new(pool).await;
    let alice = api.account("alice").await;
    let bob = api.account("bob").await;
    let project_id = collaborative_project(&api, &alice, &bob).await;
    let (_, base) = current_file(&api, &alice, &project_id).await;

    let alices = edit(&project_id, ALICE_EDIT, Some(&base));
    let bobs = edit(&project_id, BOB_EDIT, Some(&base));
    let ((first, _), (second, _)) = tokio::join!(
        api.call("POST", "/users/me/env", Some(&alice.token), alices),
        api.call("POST", "/users/me/env", Some(&bob.token), bobs),
    );
    let mut statuses = [first, second];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
}

#[sqlx::test]
async fn a_new_file_is_pushed_with_a_null_base(pool: PgPool) {
    let api = Harness::new(pool).await;
    let alice = api.account("alice").await;
    let bob = api.account("bob").await;
    let project_id = collaborative_project(&api, &alice, &bob).await;

    let mut first = edit(&project_id, ALICE_EDIT, None);
    first["filename"] = json!(".env.production");
    let (status, _) = api
        .call("POST", "/users/me/env", Some(&alice.token), first.clone())
        .await;
    assert_eq!(status, StatusCode::OK);

    // A second device creating the same file is told it already exists.
    first["content"] = json!(BOB_EDIT);
    let (status, refused) = api
        .call("POST", "/users/me/env", Some(&bob.token), first)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(refused["code"], "stale_base");
}

#[sqlx::test]
async fn viewers_cannot_read_superseded_versions(pool: PgPool) {
    let api = Harness::new(pool).await;
    let alice = api.account("alice").await;
    let bob = api.account("bob").await;
    let project_id = collaborative_project(&api, &alice, &bob).await;
    let (_, commit) = current_file(&api, &alice, &project_id).await;

    let (status, _) = api
        .call(
            "PATCH",
            &format!("/projects/{project_id}/collaborators/{}", bob.user_id),
            Some(&alice.token),
            json!({ "role": "viewer" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api
        .call(
            "GET",
            &format!("/projects/{project_id}/commits/{commit}"),
            Some(&bob.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn a_rotation_refuses_to_undo_a_push_it_did_not_see(pool: PgPool) {
    let api = Harness::new(pool).await;
    let alice = api.account("alice").await;
    let bob = api.account("bob").await;
    let project_id = collaborative_project(&api, &alice, &bob).await;
    let (_, before) = current_file(&api, &alice, &project_id).await;

    // Bob pushes while Alice is re-encrypting the version she downloaded.
    let (status, _) = api
        .call(
            "POST",
            "/users/me/env",
            Some(&bob.token),
            edit(&project_id, BOB_EDIT, Some(&before)),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let rotation = |base: &str| {
        json!({
            "grants": [
                { "user_id": alice.user_id, "wrapped_key": wrapped_for("alice", 2) },
                { "user_id": bob.user_id, "wrapped_key": wrapped_for("bob", 2) }
            ],
            "files": [{ "filename": ".env", "content": CIPHERTEXT_V2, "base_commit_id": base }],
        })
    };
    let (status, refused) = api
        .call(
            "POST",
            &format!("/projects/{project_id}/key-rotations"),
            Some(&alice.token),
            rotation(&before),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    let (content, current) = current_file(&api, &alice, &project_id).await;
    assert_eq!(content, BOB_EDIT, "Bob's push survives");

    // Re-run against what is current now, it goes through and reports the
    // commit each file moved to.
    let (status, rotated) = api
        .call(
            "POST",
            &format!("/projects/{project_id}/key-rotations"),
            Some(&alice.token),
            rotation(&current),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    let (_, after) = current_file(&api, &alice, &project_id).await;
    assert_eq!(rotated["files"][0]["filename"], ".env");
    assert_eq!(rotated["files"][0]["commit_id"], after.as_str());
}

/// A push of `content` to `.env`, edited from `base` (`None` for a new file).
fn edit(project_id: &str, content: &str, base: Option<&str>) -> Value {
    json!({
        "project_id": project_id,
        "filename": ".env",
        "content": content,
        "message": "edit",
        "key_version": 1,
        "base_commit_id": base,
    })
}

/// The ciphertext and commit id `.env` is at now.
async fn current_file(
    api: &Harness,
    account: &harness::Account,
    project_id: &str,
) -> (String, String) {
    let (status, files) = api
        .call(
            "GET",
            &format!("/users/me/env?project_id={project_id}"),
            Some(&account.token),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let file = files
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["filename"] == ".env")
        .unwrap();
    (
        file["content"].as_str().unwrap().to_string(),
        file["commit_id"].as_str().unwrap().to_string(),
    )
}

/// Alice owns a project with one `.env` file; Bob is a member. Both hold a
/// key sealed at version 1.
async fn collaborative_project(
    api: &Harness,
    alice: &harness::Account,
    bob: &harness::Account,
) -> String {
    let (_, project) = api
        .call(
            "POST",
            "/projects",
            Some(&alice.token),
            json!({ "name": "payments", "wrapped_key": wrapped_for("alice", 1) }),
        )
        .await;
    let project_id = project["project_id"].as_str().unwrap().to_string();

    api.call(
        "POST",
        "/users/me/env",
        Some(&alice.token),
        json!({
            "project_id": project_id,
            "filename": ".env",
            "content": CIPHERTEXT_V1,
            "message": "initial",
            "key_version": 1,
        }),
    )
    .await;

    api.call(
        "POST",
        &format!("/projects/{project_id}/collaborators"),
        Some(&alice.token),
        json!({ "email": bob.email, "wrapped_key": wrapped_for("bob", 1) }),
    )
    .await;
    let ott = api.email.invitation.lock().await.clone().unwrap();
    let (status, _) = api
        .call(
            "POST",
            "/projects/join",
            Some(&bob.token),
            json!({
                "ott": ott,
                "refresher_token": "b".repeat(64),
                "mac_address": "aabbccddeeff",
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    project_id
}

async fn sign_up(api: &Harness, name: &str, password: &str) -> StatusCode {
    api.call(
        "POST",
        "/auth/signup",
        None,
        json!({
            "username": name,
            "email": format!("{name}@example.com"),
            "password": password,
        }),
    )
    .await
    .0
}

async fn mailed_token(api: &Harness, email: &str) -> String {
    api.email
        .verifications
        .lock()
        .await
        .get(email)
        .cloned()
        .expect("verification token")
}

#[sqlx::test]
async fn a_verification_token_is_resent_only_to_whoever_chose_the_password(pool: PgPool) {
    let api = Harness::new(pool.clone()).await;
    let password = "a-sufficiently-long-password";
    assert_eq!(sign_up(&api, "dave", password).await, StatusCode::OK);
    let first = mailed_token(&api, "dave@example.com").await;
    // Let the per-account cooldown lapse.
    sqlx::query("UPDATE email_verifications SET created_at = now() - interval '5 minutes'")
        .execute(&pool)
        .await
        .unwrap();

    let (status, _) = api
        .call(
            "POST",
            "/auth/resend-verification",
            None,
            json!({ "email": "dave@example.com", "password": "not-the-right-password" }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the answer must not reveal a mismatch"
    );
    assert_eq!(mailed_token(&api, "dave@example.com").await, first);

    let (status, _) = api
        .call(
            "POST",
            "/auth/resend-verification",
            None,
            json!({ "email": "dave@example.com", "password": password }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let second = mailed_token(&api, "dave@example.com").await;
    assert_ne!(second, first);

    let (status, _) = api
        .call(
            "POST",
            "/auth/verify-email",
            None,
            json!({ "email": "dave@example.com", "token": first }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a resend retires the old token"
    );
    let (status, _) = api
        .call(
            "POST",
            "/auth/verify-email",
            None,
            json!({ "email": "dave@example.com", "token": second }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test]
async fn verification_resends_are_throttled(pool: PgPool) {
    let api = Harness::new(pool).await;
    let password = "a-sufficiently-long-password";
    assert_eq!(sign_up(&api, "erin", password).await, StatusCode::OK);
    let first = mailed_token(&api, "erin@example.com").await;

    let (status, _) = api
        .call(
            "POST",
            "/auth/resend-verification",
            None,
            json!({ "email": "erin@example.com", "password": password }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(mailed_token(&api, "erin@example.com").await, first);
}

#[sqlx::test]
async fn a_lapsed_unverified_signup_releases_its_address(pool: PgPool) {
    let api = Harness::new(pool.clone()).await;
    assert_eq!(
        sign_up(&api, "frank", "somebody-elses-password").await,
        StatusCode::OK
    );
    assert_eq!(
        sign_up(&api, "frank", "a-sufficiently-long-password").await,
        StatusCode::CONFLICT,
        "a pending verification still holds the address"
    );

    sqlx::query("UPDATE email_verifications SET expires_at = now() - interval '1 minute'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sign_up(&api, "frank", "a-sufficiently-long-password").await,
        StatusCode::OK
    );
    let token = mailed_token(&api, "frank@example.com").await;
    let (status, _) = api
        .call(
            "POST",
            "/auth/verify-email",
            None,
            json!({ "email": "frank@example.com", "token": token }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}
