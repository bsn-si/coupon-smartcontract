FROM paritytech/contracts-ci-linux:latest
WORKDIR /contract
COPY . .

RUN cargo contract build --release