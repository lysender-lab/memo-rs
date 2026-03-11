FROM ubuntu:24.04
WORKDIR /app

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 \
 && rm -rf /var/lib/apt/lists/*

# Expects bin/website to be mounted from host

EXPOSE 80
ENTRYPOINT ["/app/bin/website"]
