use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3000);
    let ip: std::net::IpAddr = std::env::var("BIND").ok().and_then(|b| b.parse().ok()).unwrap_or([127, 0, 0, 1].into());
    let addr = SocketAddr::new(ip, port);
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    println!("rabbit planner on http://{addr}  ({} CPU threads for simulation)", rayon::current_num_threads());
    axum::serve(listener, rabbitsim::web::router()).await.expect("serve");
}
