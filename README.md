# Evaluation

## Postgres database in docker
```bash
docker run --shm-size=1g --name rustyrts-evaluation -e POSTGRES_PASSWORD=rustyrts -p 5432:5432 -d postgres
```


## Install evaluation library
```bash
pip install .
rts_eval db postgresql://postgres:rustyrts@localhost:5432/rustyrts migrate # adapt this to your db connection
```

## Setup database scheme
```bash
rts_eval db postgresql://postgres:rustyrts@localhost:5432/mutants migrate
rts_eval db postgresql://postgres:rustyrts@localhost:5432/git migrate
```

## Dump database
```bash
rts_eval db postgresql://postgres:rustyrts@localhost:5432/mutants dump <name>
rts_eval db postgresql://postgres:rustyrts@localhost:5432/git dump <name>
```