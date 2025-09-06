use anyhow::Result;
use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use base64::{engine::general_purpose, Engine as _};
use dotenvy::dotenv;
use http_body_util::BodyExt;
use std::env;
use tracing_subscriber;
use tracing::{info, error, debug};


#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志记录器，确保控制台输出
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // 加载.env文件
    dotenv().ok();

    // 获取环境变量中的认证信息
    let username = env::var("USER")?;
    let password = env::var("PASS")?;
    let target_url = env::var("REMOTE")?;

    // 将用户名和密码编码为基本认证头部
    let credentials = format!("{}:{}", username, password);
    let encoded_credentials = general_purpose::STANDARD.encode(credentials);
    let auth_header = format!("Basic {}", encoded_credentials);

    info!("代理服务器配置加载完成");
    info!("目标服务器: {}", target_url);
    
    // 将认证信息存储在应用状态中
    let app_state = AppState {
        auth_header,
        target_url,
    };

    // 构建应用路由
    let app = Router::new()
        .route("/", any(proxy_handler))
        .route("/*path", any(proxy_handler))
        .with_state(app_state);

    // 启动服务器
    let listener = tokio::net::TcpListener::bind("127.0.0.1:11434")
        .await?;
    info!("代理服务器启动成功，监听地址: 127.0.0.1:11434");

    axum::serve(listener, app).await?;
    Ok(())
}

// 应用状态，包含认证头部和目标URL
#[derive(Clone)]
struct AppState {
    auth_header: String,
    target_url: String,
}

// 代理处理函数
async fn proxy_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: Request,
) -> Result<impl IntoResponse, AppError> {
    let request_method = req.method().clone();
    let request_path = req.uri().path().to_string();
    
    info!("接收到请求: {} {}", request_method, request_path);
    debug!("请求头: {:?}", req.headers());
    
    // 构建目标URL
    let mut target_url = state.target_url.clone();
    if let Some(path) = req.uri().path_and_query() {
        target_url.push_str(path.as_str());
    }
    
    info!("转发请求到: {}", target_url);

    // 创建HTTP客户端
    let client = reqwest::Client::new();

    // 转发请求
    let mut request_builder = client.request(req.method().clone(), &target_url);
    
    // 复制请求头部，但跳过HOST头部
    for (name, value) in req.headers() {
        // 不转发HOST头部，让reqwest自己设置
        if name != header::HOST {
            request_builder = request_builder.header(name, value);
        }
    }

    // 添加基本认证头部
    request_builder = request_builder.header(
        header::AUTHORIZATION,
        HeaderValue::from_str(&state.auth_header).map_err(anyhow::Error::from)?,
    );

    // 获取请求体
    let body_bytes = req.into_body().collect().await?.to_bytes();
    
    debug!("请求体大小: {} 字节", body_bytes.len());
    
    // 添加请求体
    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes);
    }

    // 发送请求
    let response = request_builder.send().await.map_err(|e| {
        error!("转发请求失败: {}", e);
        AppError::from(e)
    })?;

    let status = response.status();
    info!("收到目标服务器响应: 状态码 {}", status);
    debug!("响应头: {:?}", response.headers());

    // 构建响应
    let mut builder = Response::builder().status(status);

    // 复制响应头部，但过滤掉可能引起问题的头部
    for (name, value) in response.headers() {
        // 跳过可能导致问题的头部
        if name != header::TRANSFER_ENCODING && name != header::CONNECTION {
            builder = builder.header(name, value);
        }
    }

    // 获取响应体
    let body_bytes = response.bytes().await.map_err(|e| {
        error!("读取响应体失败: {}", e);
        AppError::from(e)
    })?;
    
    debug!("响应体大小: {} 字节", body_bytes.len());
    info!("请求处理完成: {} {} -> 状态码 {}", request_method, request_path, status);

    // 构建最终响应
    let response_body = Body::from(body_bytes);
    let final_response = builder.body(response_body).map_err(|e| {
        error!("构建响应失败: {}", e);
        AppError::from(e)
    })?;
    
    Ok(final_response)
}

// 自定义错误类型
#[derive(Debug)]
struct AppError(anyhow::Error);

// 错误处理
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error> + std::fmt::Display,
{
    fn from(err: E) -> Self {
        error!("处理请求时发生错误: {}", err);
        Self(err.into())
    }
}

// 实现IntoResponse特征以便在处理函数中使用
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        error!("返回错误响应: {}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("出错了: {}", self.0),
        )
            .into_response()
    }
}
