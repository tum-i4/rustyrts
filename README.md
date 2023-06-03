# Evaluation

## Install evaluation library
```bash
pip install .
```

## Start Postgres database in docker
```bash
docker run --shm-size=1g --name rustyrts-evaluation -e POSTGRES_PASSWORD=rustyrts -p 5432:5432 -d postgres
```


## Setup database scheme
Start a psql session: `psql --host=localhost --port=5432 --username=postgres`
```postgresql
CREATE database mutants;
CREATE database git;
\q
```

Migrate schema:
```bash
rts_eval db postgresql://postgres:rustyrts@localhost:5432/rustyrts migrate  # adapt this to your db connection if necessary
rts_eval db postgresql://postgres:rustyrts@localhost:5432/git migrate
```

## Start recording mutants
```bash
cd rustyrts
pyhton3 mutants.py
```


# Utilities

## Dump database
```bash
rts_eval db postgresql://postgres:rustyrts@localhost:5432/mutants dump <name>
rts_eval db postgresql://postgres:rustyrts@localhost:5432/git dump <name>
```