# Evaluation

## Prerequisites
```bash
sudo apt-get install snapd python3-dev postgresql-client
sudo snap install scc
```

## Install evaluation library
```bash
pip install -e .
```

## Start Postgres database in docker
```bash
docker run --shm-size=1g --name rustyrts-evaluation -e POSTGRES_PASSWORD=rustyrts -p 5432:5432 -d postgres
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
rustyrts_eval db postgresql://postgres:rustyrts@localhost:5432/history_sequential migrate history sequential
rustyrts_eval db postgresql://postgres:rustyrts@localhost:5432/history_parallel migrate history parallel
```

## Start recording mutants
```bash
rustyrts_eval evaluate postgresql://postgres:rustyrts@localhost:5432/mutants mutants
```


# Utilities

## Dump database
```bash
rustyrts_eval db postgresql://postgres:rustyrts@localhost:5432/<db_name> dump <name>
```