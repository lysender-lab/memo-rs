# memo-rs: A photo gallery app

`memo-rs` (memories) is a photo gallery app and a frontend for [files-rs](https://github.com/lysender/files-rs).

Written in Rust btw.

## Configuration

```
PORT=11000
SSL=false
FRONTEND_DIR=/path/to/frontend
CAPTCHA_SITE_KEY=key
CAPTCHA_SITE_SECRET=secret
JWT_SECRET=secret
API_URL=http://localhost:11001
```

## Build

Development:

```
cargo run -- server
```

With auto-rebuild:

```
cargo watch -x "run -- server"
```

Release:

```
cargo build --release
```

## Deployment

Below is an example of a simple production deployment setup using systemd.

You can deploy it however you want though.

### Setup systemd

Edit systemd service file:

First time?:

```
sudo systemctl edit --full --force memo-rs.service
```

Edit?:
```
sudo systemctl edit --full memo-rs.service
```

File: `/etc/systemd/system/memo-rs.service`

```
[Unit]
Description=memo-rs A photo gallery app

[Service]
User=www-data
Group=www-data

Environment="PORT=11000"
Environment="SSL=false"
Environment="FRONTEND_DIR=/data/www/html/sites/memo-rs/frontend"
Environment="CAPTCHA_SITE_KEY=key"
Environment="CAPTCHA_SITE_SECRET=secret"
Environment="JWT_SECRET=secret"
Environment="API_URL=http://localhost:11001"

WorkingDirectory=/data/www/html/sites/memo-rs/
ExecStart=/data/www/html/sites/memo-rs/target/release/memo-rs
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```

To enable it for the first time:

```
sudo systemctl enable memo-rs.service
```

Various commands:

```
sudo systemctl start memo-rs.service
sudo systemctl stop memo-rs.service
sudo systemctl restart memo-rs.service
sudo systemctl status memo-rs.service
```

### nginx

nginx config:

```nginx configuration
server {
    listen 80;
    server_name memories-domain.com;
    access_log off;
    error_log off;
    ## redirect http to https ##
    return      301 https://memories-domain.com$request_uri;
}

server {
    listen 443 ssl;
    server_name memories-domain.com;

    ssl_certificate     /etc/nginx/certs/memories-domain.com/server.crt;
    ssl_certificate_key /etc/nginx/certs/memories-domain.com/server.key;

    root /data/www/html/sites/memories-domain/frontend/public;

    # Need to find a way to cache all static contents either in nginx or in rust/axum/tower
    location ~* \.(ico|css|js|gif|jpeg|jpg|png|woff|ttf|otf|svg|woff2|eot)$ {
      expires 1y;
      add_header Cache-Control public;

      add_header ETag "";
    }

    location / {
        proxy_pass         http://127.0.0.1:11000/;
        proxy_redirect     off;

        proxy_set_header   Host             $host;
        proxy_set_header   X-Real-IP        $remote_addr;
        proxy_set_header   X-Forwarded-For  $proxy_add_x_forwarded_for;

        client_max_body_size       10m;
        client_body_buffer_size    128k;

        proxy_connect_timeout      90;
        proxy_send_timeout         90;
        proxy_read_timeout         90;

        proxy_buffer_size          8k;
        proxy_buffers              4 64k;
        proxy_busy_buffers_size    128k;
        proxy_temp_file_write_size 128k;
    }
}
```
