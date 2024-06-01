#[tokio::main]
pub async fn main() {
    // The response we get after logging in
    let login_resp = Some("serverversion:2004&...");
    // The url we want to know the details of
    let url = "https://f8.sfgame.net/req.php?req=0...";
    let decrypted = sf_api::session::decrypt_url(url, login_resp).unwrap();
    println!("{decrypted:#?}");
}
