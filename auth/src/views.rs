/// Project Raindrops login page HTML
pub fn login_page_html(enable_plain: bool, enable_bluesky: bool, enable_github: bool, enable_google: bool, enable_linkedin: bool) -> String {
    let mut providers_html = String::new();
    
    if enable_plain {
        providers_html.push_str(r#"
        <div class="provider-section">
            <h3>Login with Username & Password</h3>
            <form method="post" action="/auth/plain/login">
                <div class="form-group">
                    <input type="text"
                           name="username"
                           placeholder="Username"
                           required>
                </div>
                <div class="form-group">
                    <input type="password"
                           name="password"
                           placeholder="Password"
                           required>
                </div>
                <button type="submit" class="btn btn-primary">Login</button>
            </form>
        </div>
        "#);
    }
    
    if enable_bluesky {
        providers_html.push_str(r#"
        <div class="provider-section">
            <h3>Login with Bluesky</h3>
            <p>Enter your Bluesky handle to authenticate:</p>
            <form id="loginForm" method="post" action="/auth/bluesky/login">
                <div class="form-group">
                    <input type="text"
                           name="handle"
                           placeholder="@alice.bsky.social"
                           pattern="@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
                           title="Enter a valid Bluesky handle (e.g., @alice.bsky.social)"
                           required>
                </div>
                <button type="submit" class="btn btn-bluesky">Login with Bluesky</button>
            </form>
            <div id="error-message" class="error" style="display: none;"></div>
        </div>
        "#);
    }
    
    if enable_github {
        providers_html.push_str(r#"
        <div class="provider-section">
            <h3>Login with GitHub</h3>
            <a href="/auth/github/login" class="btn btn-github">
                <svg height="16" width="16" style="vertical-align: text-bottom; margin-right: 8px;" viewBox="0 0 16 16" fill="white">
                    <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"></path>
                </svg>
                Login with GitHub
            </a>
        </div>
        "#);
    }
    
    if enable_google {
        providers_html.push_str(r#"
        <div class="provider-section">
            <h3>Login with Google</h3>
            <a href="/auth/google/login" class="btn btn-google">
                <svg height="16" width="16" style="vertical-align: text-bottom; margin-right: 8px;" viewBox="0 0 24 24">
                    <path fill="white" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
                    <path fill="white" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
                    <path fill="white" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
                    <path fill="white" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
                </svg>
                Login with Google
            </a>
        </div>
        "#);
    }
    
    if enable_linkedin {
        providers_html.push_str(r#"
        <div class="provider-section">
            <h3>Login with LinkedIn</h3>
            <a href="/auth/linkedin/login" class="btn btn-linkedin">
                <svg height="16" width="16" style="vertical-align: text-bottom; margin-right: 8px;" viewBox="0 0 24 24" fill="white">
                    <path d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433c-1.144 0-2.063-.926-2.063-2.065 0-1.138.92-2.063 2.063-2.063 1.14 0 2.064.925 2.064 2.063 0 1.139-.925 2.065-2.064 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z"/>
                </svg>
                Login with LinkedIn
            </a>
        </div>
        "#);
    }
    
    format!(r#"<!DOCTYPE html>
<html>
<head>
    <title>Login - Project Raindrops</title>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}
        
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 20px;
        }}
        
        .login-container {{
            background: white;
            border-radius: 16px;
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
            max-width: 500px;
            width: 100%;
            padding: 40px;
        }}
        
        .header {{
            text-align: center;
            margin-bottom: 40px;
        }}
        
        .logo {{
            font-size: 48px;
            margin-bottom: 10px;
        }}
        
        h1 {{
            color: #333;
            font-size: 28px;
            margin-bottom: 8px;
        }}
        
        .subtitle {{
            color: #666;
            font-size: 14px;
        }}
        
        .provider-section {{
            margin-bottom: 30px;
            padding-bottom: 30px;
            border-bottom: 1px solid #e0e0e0;
        }}
        
        .provider-section:last-child {{
            border-bottom: none;
            margin-bottom: 0;
            padding-bottom: 0;
        }}
        
        h3 {{
            color: #333;
            font-size: 16px;
            margin-bottom: 10px;
        }}
        
        p {{
            color: #666;
            font-size: 14px;
            margin-bottom: 15px;
        }}
        
        .form-group {{
            margin: 15px 0;
        }}
        
        input[type="text"],
        input[type="password"] {{
            width: 100%;
            padding: 12px 16px;
            border: 2px solid #e0e0e0;
            border-radius: 8px;
            font-size: 16px;
            transition: border-color 0.3s ease;
        }}
        
        input[type="text"]:focus,
        input[type="password"]:focus {{
            border-color: #667eea;
            outline: none;
        }}
        
        .btn {{
            display: inline-flex;
            align-items: center;
            justify-content: center;
            width: 100%;
            padding: 12px 24px;
            border: none;
            border-radius: 8px;
            font-size: 16px;
            font-weight: 600;
            text-decoration: none;
            cursor: pointer;
            transition: transform 0.2s ease, box-shadow 0.2s ease;
        }}
        
        .btn:hover {{
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
        }}
        
        .btn:active {{
            transform: translateY(0);
        }}
        
        .btn-primary {{
            background-color: #667eea;
            color: white;
        }}
        
        .btn-bluesky {{
            background-color: #0085ff;
            color: white;
        }}
        
        .btn-github {{
            background-color: #24292e;
            color: white;
        }}
        
        .btn-google {{
            background-color: #4285f4;
            color: white;
        }}
        
        .btn-linkedin {{
            background-color: #0077b5;
            color: white;
        }}
        
        .error {{
            color: #dc3545;
            background-color: #f8d7da;
            border: 1px solid #f5c6cb;
            border-radius: 8px;
            padding: 12px;
            margin-top: 15px;
            font-size: 14px;
        }}
        
        .footer {{
            text-align: center;
            margin-top: 30px;
            padding-top: 20px;
            border-top: 1px solid #e0e0e0;
        }}
        
        .footer p {{
            color: #999;
            font-size: 12px;
        }}
    </style>
</head>
<body>
    <div class="login-container">
        <div class="header">
            <div class="logo">🌧️</div>
            <h1>Project Raindrops</h1>
            <p class="subtitle">Secure Authentication</p>
        </div>
        
        {}
        
        <div class="footer">
            <p>Powered by WiFi Verify Authentication</p>
        </div>
    </div>
    
    <script>
        const loginForm = document.getElementById('loginForm');
        if (loginForm) {{
            loginForm.addEventListener('submit', async function(e) {{
                e.preventDefault();
                const formData = new FormData(this);
                const errorDiv = document.getElementById('error-message');
                
                try {{
                    const response = await fetch('/auth/bluesky/login', {{
                        method: 'POST',
                        headers: {{
                            'Content-Type': 'application/x-www-form-urlencoded',
                        }},
                        body: new URLSearchParams(formData)
                    }});
                    
                    if (!response.ok) {{
                        const errorText = await response.text();
                        errorDiv.textContent = errorText || 'Login failed';
                        errorDiv.style.display = 'block';
                        return;
                    }}
                    
                    const data = await response.json();
                    window.location.href = data.auth_url;
                    
                }} catch (error) {{
                    errorDiv.textContent = 'Network error: ' + error.message;
                    errorDiv.style.display = 'block';
                }}
            }});
        }}
    </script>
</body>
</html>"#, providers_html)
}

/// Access denied page HTML for users who authenticated but are not in the allowed list
pub fn access_denied_page_html(user_handle: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html>
<head>
    <title>Access Denied - Project Raindrops</title>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}
        
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 20px;
        }}
        
        .container {{
            background: white;
            border-radius: 16px;
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
            max-width: 500px;
            width: 100%;
            padding: 40px;
            text-align: center;
        }}
        
        .icon {{
            font-size: 64px;
            margin-bottom: 20px;
        }}
        
        h1 {{
            color: #dc3545;
            font-size: 28px;
            margin-bottom: 16px;
        }}
        
        p {{
            color: #666;
            font-size: 16px;
            line-height: 1.6;
            margin-bottom: 12px;
        }}
        
        .user-info {{
            background-color: #f8f9fa;
            border-radius: 8px;
            padding: 16px;
            margin: 24px 0;
            word-break: break-word;
        }}
        
        .user-handle {{
            font-weight: 600;
            color: #333;
            font-family: monospace;
        }}
        
        .btn {{
            display: inline-block;
            margin-top: 24px;
            padding: 12px 24px;
            background-color: #667eea;
            color: white;
            text-decoration: none;
            border-radius: 8px;
            font-weight: 600;
            transition: transform 0.2s ease, box-shadow 0.2s ease;
        }}
        
        .btn:hover {{
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
        }}
        
        .footer {{
            margin-top: 32px;
            padding-top: 20px;
            border-top: 1px solid #e0e0e0;
        }}
        
        .footer p {{
            color: #999;
            font-size: 12px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="icon">🚫</div>
        <h1>Access Denied</h1>
        <p>You have successfully authenticated, but your account does not have access to this application.</p>
        
        <div class="user-info">
            <p style="margin-bottom: 8px; color: #666; font-size: 14px;">Authenticated as:</p>
            <p class="user-handle">{}</p>
        </div>
        
        <p>If you believe this is an error, please contact your system administrator.</p>
        
        <form method="post" action="/auth/logout">
            <button type="submit" class="btn">Logout</button>
        </form>
        
        <div class="footer">
            <p>Project Raindrops</p>
        </div>
    </div>
</body>
</html>"#, user_handle)
}
