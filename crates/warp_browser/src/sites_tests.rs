use super::{ApprovedSites, LOCAL_FILES_SITE, site_requiring_approval};

#[test]
fn local_addresses_need_no_approval() {
    assert_eq!(site_requiring_approval("http://localhost:5173/app"), None);
    assert_eq!(site_requiring_approval("http://127.0.0.1:8080/"), None);
    assert_eq!(site_requiring_approval("http://app.localhost:4000/"), None);
}

#[test]
fn pages_that_are_not_websites_need_no_approval() {
    assert_eq!(site_requiring_approval("about:blank"), None);
    assert_eq!(site_requiring_approval(""), None);
}

#[test]
fn local_files_need_approval() {
    assert_eq!(
        site_requiring_approval("file:///Users/me/.ssh/id_rsa"),
        Some(LOCAL_FILES_SITE.to_owned())
    );
}

#[test]
fn remote_sites_need_approval_by_host_without_www() {
    assert_eq!(
        site_requiring_approval("https://www.github.com/warpdotdev"),
        Some("github.com".to_owned())
    );
    assert_eq!(
        site_requiring_approval("https://docs.rs/wry"),
        Some("docs.rs".to_owned())
    );
}

#[test]
fn approved_sites_round_trip_through_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("browser-agent-sites.json");
    let mut sites = ApprovedSites::default();
    sites.approve("github.com".to_owned());

    sites.save(&path).unwrap();
    let loaded = ApprovedSites::load(&path);

    assert!(loaded.allows("github.com"));
    assert!(!loaded.allows("docs.rs"));
}

#[test]
fn auto_approve_allows_every_site() {
    let mut sites = ApprovedSites::default();

    sites.set_auto_approve(true);

    assert!(sites.allows("docs.rs"));
}

#[test]
fn auto_approve_is_saved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("browser-agent-sites.json");
    let mut sites = ApprovedSites::default();
    sites.set_auto_approve(true);

    sites.save(&path).unwrap();

    assert!(ApprovedSites::load(&path).auto_approve());
}
