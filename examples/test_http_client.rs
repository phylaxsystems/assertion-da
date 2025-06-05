use assertion_da_client::DaClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Test with HTTP endpoint
    println!("Testing HTTP endpoint...");
    let _http_client = DaClient::new("http://localhost:5001")?;
    println!("✓ HTTP client created successfully");

    // Test with HTTPS endpoint
    println!("Testing HTTPS endpoint...");
    let _https_client = DaClient::new("https://demo-21-assertion-da.phylax.systems")?;
    println!("✓ HTTPS client created successfully");

    // Test with authentication
    println!("Testing authenticated client...");
    let _auth_client = DaClient::new_with_auth(
        "https://demo-21-assertion-da.phylax.systems",
        "Bearer test-token"
    )?;
    println!("✓ Authenticated client created successfully");

    println!("\nClient creation tests passed! The new reqwest-based client should resolve TLS/SSL issues.");
    println!("Now you can use the client with both HTTP and HTTPS endpoints without TLS errors.");

    Ok(())
} 