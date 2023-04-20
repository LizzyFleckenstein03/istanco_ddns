use anyhow::anyhow;
use scraper::{Html, Selector};

async fn form_get(
    client: &reqwest::Client,
    addr: &str,
    form: &str,
) -> anyhow::Result<Vec<(String, String)>> {
    Ok(
        Html::parse_document(&client.get(addr).send().await?.text().await?)
            .select(&Selector::parse(form).unwrap())
            .next()
            .ok_or(anyhow!("form for selector {form} not found"))?
            .select(&Selector::parse("input").unwrap())
            .map(|item| item.value())
            .flat_map(|item| {
                if matches!(item.attr("type"), Some(x) if x == "checkbox" || x == "submit") {
                    return None;
                }

                Some((
                    item.attr("name")?.into(),
                    item.attr("value").unwrap_or_default().into(),
                ))
            })
            .collect(),
    )
}

// this is dumb but it works
// we don't use a hashmap because keys may be duplicated
// this overwrites the first entry with a certain key
fn form_set(form: &mut Vec<(String, String)>, key: &str, value: String) {
    for entry in form.iter_mut() {
        if entry.0 == key {
            entry.1 = value;
            return;
        }
    }

    form.push((key.into(), value));
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;

    use std::env;

    let target_host = env::var("DDNS_TARGET_HOST")?;
    let username = env::var("DDNS_USERNAME")?;
    let password = env::var("DDNS_PASSWORD")?;
    let domain_id = env::var("DDNS_DOMAIN_ID")?;

    let target_ip = *dns_lookup::lookup_host(&target_host)?
        .first()
        .ok_or(anyhow!("no ip found for {target_host}"))?;

    let client = reqwest::ClientBuilder::new().cookie_store(true).build()?;

    let mut login = form_get(
        &client,
        "https://cp.istanco.net/clientarea.php",
        r#"form[action="https://cp.istanco.net/dologin.php"]"#,
    )
    .await?;

    form_set(&mut login, "username", username);
    form_set(&mut login, "password", password);

    client
        .post("https://cp.istanco.net/dologin.php")
        .form(&login)
        .send()
        .await?;

    let manage_url = format!("https://cp.istanco.net/index.php?m=br_dnsmanager&id={domain_id}");

    let mut manage = form_get(&client, &manage_url, r#"form[name="br-dnsrecord-manager"]"#).await?;

    form_set(&mut manage, "value[]", target_ip.to_string());
    form_set(&mut manage, "btnSave", "Save Changes".into());

    client.post(&manage_url).form(&manage).send().await?;

    Ok(())
}
