import pandas as pd
from sqlalchemy import create_engine

from rustyrts.git.plots.scripts.labels import get_labels_git, url_git

table_ddl = """
create sequence "AnalysisTestsNotContained_id_seq"
    as integer;

alter sequence "AnalysisTestsNotContained_id_seq" owner to postgres;

create table "AnalysisTestsNotContained"
(
    id                 integer default nextval('"AnalysisTestsNotContained_id_seq"'::regclass) not null
        constraint "AnalysisTestsNotContained_pk"
            primary key,
    repo_id            integer                                                                not null
        constraint "AnalysisTestsNotContained_Repoistory_null_fk"
            references "Repository",
    commit             integer                                                                not null
        constraint "AnalysisTestsNotContained_Commit_null_fk"
            references "Commit",
    test_name          varchar                                                                not null,
    reason             varchar,
    comment            varchar
);

alter table "AnalysisTestsNotContained"
    owner to postgres;

"""

labels = get_labels_git()


def get_test_diff(retest_all, other):
    retest_all_tests = retest_all.splitlines()
    other_tests = other.splitlines()
    return list(set(retest_all_tests) - set(other_tests))


df_selected_dynamic = pd.read_sql(
    'SELECT c.repo_id as repository, commit, dynamic FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
    url_git)

df_selected_static = pd.read_sql(
    'SELECT c.repo_id as repository, commit, static FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
    url_git)

not_selected_static = {}

for i in range(1, len(labels) + 1):
    not_selected_static[i] = {}

selected_dynamic = df_selected_dynamic.to_dict(orient='records')
selected_static = df_selected_static.to_dict(orient='records')

assert len(selected_static) == len(selected_static)

for (dynamic_report, static_report) in zip(selected_dynamic, selected_static):
    assert dynamic_report['commit'] == static_report['commit']

    repository = static_report['repository']
    commit = static_report['commit']

    diff = get_test_diff(dynamic_report['dynamic'], static_report['static'])

    if commit not in not_selected_static[repository]:
        not_selected_static[repository][commit] = []

    for test in diff:
        not_selected_static[repository][commit].append(test)

engine = create_engine(url_git)
with engine.connect() as conn:
    conn.execute(table_ddl)

    for repository in not_selected_static:
        for commit in not_selected_static[repository]:
            for test in not_selected_static[repository][commit]:
                statement = f"INSERT INTO public.\"AnalysisTestsNotContained\" (repo_id, commit, test_name) VALUES ({repository}, {commit}, '{test}')"
                conn.execute(statement)
