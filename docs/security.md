### Security

This appservice is designed to make matrix room data public. These rooms must explicitly have a `world_visible` history visibilty state event. In addition, the appservice virtual user must be explicitly invited to every room that is to be made public. To add another layer of permissions, these rooms must have a custom state event of type `commune.room.public` with `content` set to `public: true`. 

In other words, simply running this appservice alongside a matrix homeserver will not leak any room data. All access rules must be explicitly set beforehand.

### Public Access
The appservice cannot read events from rooms it hasn't been invited to. The routing middleware [handler](https://github.com/commune-sh/public-appservice/blob/aacdb2982cdc2722460edeec2011c6b21c0019fe/src/middleware.rs#L200) enforces this on every API route.

It also explicitly rejects joining and accessing DM rooms, and rooms with E2EE enabled. 

### API 

The API surface provided by this appservice are a subset of GET requests from the [matrix client-server-api](https://spec.matrix.org/latest/client-server-api/). There is no way for the appservice to send new events or modify existing events in rooms that it has access to. 

The endpoints are the absolutely necessary ones needed to build a public rendering of the chatroom view - these include room messages, room state, individual events, threads, space hierarchies, event relationships and media.

The full list of API routes can be found [here](https://github.com/commune-sh/public-appservice/blob/aacdb2982cdc2722460edeec2011c6b21c0019fe/src/server.rs#L85).

### Federation
A homeserver running this appservice can make remote federated rooms on other homeservers publicly available too, with [checks in place to ensure](https://github.com/commune-sh/public-appservice/blob/aacdb2982cdc2722460edeec2011c6b21c0019fe/src/api.rs#L172) it doesn't happen accidentally or without permission. The appservice has a configuration option that allows only local users to invite the public appservice - meaning users on homeserver B cannot invite homeserver A's appservice user to rooms on the remote homeserver. 

Additionally, federated homeserver domains can be [whitelisted](https://github.com/commune-sh/public-appservice/blob/aacdb2982cdc2722460edeec2011c6b21c0019fe/src/api.rs#L168) to ensure only a limited set of remote rooms are publicly accessible. 

The appservice membership and join [code](https://github.com/commune-sh/public-appservice/blob/aacdb2982cdc2722460edeec2011c6b21c0019fe/src/api.rs#L89) controls most of the mechanism behind these rules. 

### Audit
This code has not been audited or reviewed by external parties yet, so this document should be viewed as a preliminary outline of all the measures taken to ensure good and necessary security practice. We're hoping to have help with this when possible, and will update the code as is necessary.
