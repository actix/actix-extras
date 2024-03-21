# Examples

We leverage redis to store state of the ratelimiting.  
So you will need to have a redis instance available on localhost.  

You can start this redis instance with Docker:
```
docker run -d -p 6379:6379 --name limiter-redis redis
# Clean up: you can rm the docker this way
# docker rm -f limiter-redis
```


## scoped_limiters

This example present how to use multiple limiters.
This allow different configurations and the ability to scope them.

### Starting the example server

```bash
RUST_LOG=debug cargo run --example scoped_limiters
```
> RUST_LOG=debug is used to print logs, see crate pretty_env_logger for more details.

### Testing with curl

```bash
curl -X PUT localhost:8080/scoped/sms -v
```
first request should work fine  
doing a second request within 60 seconds should yield `HTTP/1.1 429 Too Many Requests`  
after 60 seconds you should be able to make 1 request again


```bash
curl localhost:8080
```
This route should work 30 times, or 29 if you previously requested the /scoped/sms route
