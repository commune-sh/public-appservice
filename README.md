### Public Appservice

This is an appservice for making matrix rooms and spaces publicly accessible - intended
to be used with [Commune](https://github.com/commune-sh/commune).

The appservice user joins any public matrix rooms it's invited to, and the server proxies specific read-only endpoints to the homeserver's REST API, using the appservice token. 

This is a work in progress, and has been tested with Synapse, Dendrite and
Conduit. It's still rough around the edges, but can be used in production. It's
currently running on the [Commune](https://commune.sh) and [Dev](https://dev.commune.sh) instances.

#### Discovery

The Commune client queries the matrix homeserver's `/.well-known/matrix/client` endpoint to detect whether this appservice is running. Ensure that the endpoint returns the `public.appservice` URL:

```json
{
  "m.homeserver": {
    "base_url": "https://matrix.commune.sh"
  },
  "public.appservice": {
    "url": "https://public.commune.sh"
  },
}
```

If you're running Synapse, this can be served by adding the following to you
`homeserver.yaml`:

```yaml
extra_well_known_client_content :
  public.appservice: 
    url: "https://public.commune.sh"
```

It's probably better serve this from a reverse , or something like a Cloudflare
worker route.

#### Configuration

Register a new appservice on your Synapse homeserver:

```yaml
id: "commune_public_access"
url: "http://localhost:8989"
as_token: "app_service_access_token"
hs_token: "homeserver_access_token"
sender_localpart: "public" 
rate_limited: false
namespaces:
  rooms:
  - exclusive: false
    regex: "!.*:.*"
```

For alternative server implementations like Dendrite or Conduit, look up the relevant appservice configuration documentation.

Copy `config.sample.toml` to `config.toml` and fill in the required fields.

```toml
[app]
port = 8989
allow_origin = [""]

[appservice]
id = "commune"
sender_localpart = "public"
access_token = "app_service_access_token"
hs_access_token = "homeserver_access_token"

[appservice.rules]
auto_join = true
invite_by_local_user = true
federation_domain_whitelist = ["matrix.org", "dev.commune.sh"]

[matrix]
homeserver = "http://localhost:8080"
server_name = "localhost:8480"

[redis]
address = "localhost:6379"
password = ""
rooms_db = 1
messages_db = 2
events_db = 3
state_db = 4

[cache.public_rooms]
enabled = true
expire_after = 14400

[cache.room_state]
enabled = true
expire_after = 3600

[cache.messages]
enabled = true
expire_after = 3600

```

To ensure that this appservice only joins local homeserver rooms, leave the `federation_domain_whitelist` value empty. Otherwise fill in the domains you want to allow. Additionally, the appservice can be limited to join rooms by local usersonly by setting `invite_by_local_user` to `true`.

#### Dependencies

This appservice uses redis to cache public room data. Ensure that you have a redis server running and accessible to the appservice.

#### Running

There are a couple of ways to run this appservice. You can clone the repo and
build it with `cargo build --release` and run the binary with `./target/release/public-appservice --config=/path/to/config.toml`.

You can also install it with `cargo install public-appservice` and run it with `public-appservice --config=/path/to/config.toml`.

Additionally, you can run the server in a container with `docker compose up -v`.

#### Deploying

For simplicity, run this appservice on the same host where the matrix homeserver lives, although it isn't necessary. There are example docs for both a systemd unit and nginx reverse proxy in the [`/docs`](https://github.com/commune-sh/appservice/tree/main/docs).

### Development

To develop this appservice, you'll need to have a matrix homeserver running locally. Update the `config.toml` file to point to your locally running matrix instance. Run `cargo run` to start the appservice.

#### Community

To keep up to date with Commune development, you can find us on `#commune:commune.sh` or `#commune:matrix.org`.

#### Funding

This project is funded through [NGI0 Entrust](https://nlnet.nl/entrust), a fund established by [NLnet](https://nlnet.nl) with financial support from the European Commission's [Next Generation Internet](https://ngi.eu) program. Learn more at the [NLnet project page](https://nlnet.nl/project/Commune).

[<img src="https://nlnet.nl/logo/banner.png" alt="NLnet foundation logo" width="20%" />](https://nlnet.nl)
[<img src="https://nlnet.nl/image/logos/NGI0_tag.svg" alt="NGI Zero Logo" width="20%" />](https://nlnet.nl/entrust)

