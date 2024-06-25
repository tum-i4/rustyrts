# Evaluation

## Prerequisites

- Rustup
- Python3.10
- Docker

- rustyrts (branch develop/main)
- mutants-rts (branch mutants-rts)

- Non-exhaustive list of other packages that may be required to run the evaluation or compile the projects:

```bash
sudo apt-get install snapd python3-dev postgresql-client python3-pip python3.10-venv
sudo apt-get install gcc lld libssl-dev cmake protobuf-compiler clang libsqlite3-dev
sudo apt-get install build-essential libsnappy-dev zlib1g-dev libbz2-dev libgflags-dev liblz4-dev libzstd-dev
sudo apt-get install git-lfs
sudo snap install scc
```

## Set default toolchain

```bash
rustup default nightly-2023-12-28-x86_64-unknown-linux-gnu
rustup toolchain uninstall stable-x86_64-unknown-linux-gnu
```

## Install evaluation library (inside repository on branch evaluation)

```bash
pip install -e .
```

## Start Postgres database in docker

```bash
sudo docker run --shm-size=1g --name rustyrts-evaluation -e POSTGRES_PASSWORD=rustyrts -p 5432:5432 -d postgres:12-bookworm
```

## Setup database scheme

Start a psql session: `psql --host=localhost --port=5432 --username=postgres`

```postgresql
CREATE database mutants;
CREATE database history_sequential;
CREATE database history_parallel;
\q
```

Migrate schema:

```bash
rustyrts_eval db postgresql://postgres:rustyrts@localhost:5432/mutants migrate mutants  # adapt this to your db connection if necessary
rustyrts_eval db postgresql://postgres:rustyrts@localhost:5432/history_sequential migrate history_sequential
rustyrts_eval db postgresql://postgres:rustyrts@localhost:5432/history_parallel migrate history_parallel
```

## Start recording mutants

```bash
rustyrts_eval evaluate postgresql://postgres:rustyrts@localhost:5432/mutants mutants
```

## Start recording history changes

```bash
rustyrts_eval evaluate postgresql://postgres:rustyrts@localhost:5432/history_parallel history hardcoded parallel
rustyrts_eval evaluate postgresql://postgres:rustyrts@localhost:5432/history_sequential history hardcoded sequential
```

## Analyze results

```bash
rustyrts_eval analyze postgresql://postgres:rustyrts@localhost:5432/mutants mutants
rustyrts_eval analyze postgresql://postgres:rustyrts@localhost:5432/history_parallel history
rustyrts_eval analyze postgresql://postgres:rustyrts@localhost:5432/history_sequential history
```

# Utilities

## Dump database

```bash
rustyrts_eval db postgresql://postgres:rustyrts@localhost:5432/<db_name> dump <file_name>
```

## Restore database backup

```bash
rustyrts_eval db postgresql://postgres:rustyrts@localhost:5432/<db_name> restore <file_name>
```
