use futures::{Future, future::{BoxFuture, FutureExt}};
use hyper::{body::HttpBody as _, client::HttpConnector, Body, Client, Method, Request};
use hyper_tls::HttpsConnector;
use scraper::{Html, Selector};
use tokio::task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
     // I hope this is the right way to do it
    let client = Box::leak( Box::new(Client::builder().build::<_, hyper::Body>(HttpsConnector::new())));
    let ilias_tree = load_ilias(
        "ilias.php?ref_id=1836117&cmdClass=ilrepositorygui&cmdNode=yj&baseClass=ilrepositorygui"
            .to_string(),
        "Rechnernetze".to_string(),
        client,
    )
    .await?;
    println!("{:#?}", ilias_tree);
    Ok(())
}

async fn request_il_page(
    uri: String,
    client: &Client<HttpsConnector<HttpConnector>>,
) -> Result<Html, Box<dyn std::error::Error + Send + Sync>> {
    let req = Request::builder()
        .method(Method::GET)
        .uri("https://ilias.uni-freiburg.de/".to_owned() + &uri)
        .header("authority", "ilias.uni-freiburg.de")
        .header("upgrade-insecure-requests", 1)
        .header("dnt", 1)
        .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36")
        .header("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.9")
        .header("sec-fetch-site", "same-origin")
        .header("sec-fetch-mode", "navigate")
        .header("sec-fetch-user", "?1")
        .header("sec-fetch-dest", "document")
        .header("referer", "https://ilias.uni-freiburg.de/ilias.php?baseClass=ilPersonalDesktopGUI&cmd=jumpToSelectedItems")
        .header("accept-language", "de-DE,de;q=0.9,en-US;q=0.8,en;q=0.7")
        .header("cookie", "iom_consent=00000000000000&1604408733754; ioam2018=000ef5c0cec6382585fa1559e:1632834334057:1604408734057:.uni-freiburg.de:6:ak025:dbs:noevent:1604871141009:nb8xt2; ilClientId=unifreiburg; _shibsession_64656661756c7468747470733a2f2f696c6961732e756e692d66726569627572672e64652f73686962626f6c657468=_0ed19f012091e81c7f66970654a4def0; PHPSESSID=qrb2h55lg6hh17cn9ckmnpiid0")
        .body(Body::empty()).unwrap();

    let mut resp = client.request(req).await?;
    let mut bytes = vec![];
    while let Some(chunk) = resp.body_mut().data().await {
        let chunk = chunk?;
        bytes.extend(&chunk[..]);
    }
    Ok(Html::parse_document(std::str::from_utf8(&bytes)?))
}



static CONTAINERS: Selector = Selector::parse(".ilContainerListItemOuter .il_ContainerItemTitle a").unwrap();

async fn get_child_pages(uri: String, client: &Client<HttpsConnector<HttpConnector>>) -> Vec<(String, String)>{
    let html = request_il_page(uri, client).await.unwrap();
    let elements = html.select(&CONTAINERS);
    let mut element_infos = vec![];
    for element in elements {
        element_infos.push((
            element.value().attr("href").unwrap().to_string(),
            element.inner_html(),
        ))
    }
    element_infos
    
}

#[derive(Debug)]
enum IlNodeType {
    Forum,
    Folder,
    File
}

fn get_il_node_type(uri: &str) -> IlNodeType{
    return IlNodeType::Folder
}

#[derive(Debug)]
struct IlNode {
    title: String,
    children: Option<Vec<IlNode>>,
    uri: String,
    path: String,  // disc path
    sync: bool,    // should this node be synced 
    breed: IlNodeType,
}

fn load_ilias(
    uri: String,
    title: String,
    path: String,
    client: &'static Client<HttpsConnector<HttpConnector>>,
) -> tokio::task::JoinHandle<IlNode> {
    task::spawn(async move {
        
        let node_type = get_il_node_type(&uri);

        let mut node = match node_type {
            IlNodeType::Forum => {
                IlNode{
                    title,
                    children: None,
                    path: path + '/' + title,
                    sync: false,
                    breed: IlNodeType::Forum,
                    uri
                }
            }
            IlNodeType::Folder => {
                let child_elements = get_child_pages(uri, client).await;
                // create children
                let mut handles = vec![];
                for element in child_elements {
                    handles.push(load_ilias(element.0, element.1, &client));
                }

                // load children and add them to the node
                let mut children = vec![];
                for handle in handles {
                    children.push(handle.await.unwrap());
                }
                IlNode{
                    title,
                    children,
                    path: path + '/' + title,
                    sync: false,
                    breed: IlNodeType::Folder,
                    uri
                }
            }
            IlNodeType::File => {
                let endung = "pdf";  //TODO: change that
                IlNode{
                    title,
                    children: None,
                    path: path + '/' + title + endung,
                    sync: true,
                    breed: IlNodeType::File,
                    uri
                }
            }
        };
        node
    })
}
