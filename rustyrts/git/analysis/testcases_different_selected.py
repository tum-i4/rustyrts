import pandas as pd
from sqlalchemy import create_engine

from rustyrts.git.plots._scripts.labels import get_labels_git, url_git

table_ddl = """
create sequence "AnalysisDifferentSelected_id_seq"
    as integer;

alter sequence "AnalysisDifferentSelected_id_seq" owner to postgres;

create table "AnalysisDifferentSelected"
(
    id                 integer default nextval('"AnalysisDifferentSelected_id_seq"'::regclass) not null
        constraint "AnalysisDifferentSelected_pk"
            primary key,
    repo_id            integer                                                                not null
        constraint "AnalysisTestsNotContained_Repoistory_null_fk"
            references "Repository",
    commit             integer                                                                not null
        constraint "AnalysisDifferentSelected_Commit_null_fk"
            references "Commit",
    test_name          varchar                                                                not null,
    parent_result      teststatus,
    result         teststatus,
    reason             varchar,
    comment            varchar,
    algorithm          varchar                                                                not null
);

alter table "AnalysisDifferentSelected"
    owner to postgres;

"""

labels = get_labels_git()


def get_test_intersection(retest_all, other):
    retest_all_tests = set(retest_all.splitlines())
    other_tests = set(other.splitlines())
    return list(retest_all_tests.intersection(other_tests))


df_different_retest_all = pd.read_sql(
    'SELECT c.repo_id as repository, commit, retest_all_different FROM testcases_newly_different join "Commit" c ON c.id = commit ORDER BY commit',
    url_git)

df_selected_dynamic = pd.read_sql(
    'SELECT c.repo_id as repository, commit, dynamic FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
    url_git)

df_selected_static = pd.read_sql(
    'SELECT c.repo_id as repository, commit, static FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
    url_git)

tests_selected_dynamic = {}
tests_selected_static = {}

for i in range(1, len(labels) + 1):
    tests_selected_dynamic[i] = {}
    tests_selected_static[i] = {}

different_retest_all = df_different_retest_all.to_dict(orient='records')
selected_dynamic = df_selected_dynamic.to_dict(orient='records')
selected_static = df_selected_static.to_dict(orient='records')

assert len(different_retest_all) == len(selected_dynamic) and len(different_retest_all) == len(selected_static)

for (retest_all_report, dynamic_report, static_report) in zip(different_retest_all, selected_dynamic, selected_static):
    assert retest_all_report['commit'] == dynamic_report['commit']
    assert retest_all_report['commit'] == static_report['commit']

    repository = retest_all_report['repository']
    commit = retest_all_report['commit']

    diff_dynamic = get_test_intersection(retest_all_report['retest_all_different'], dynamic_report['dynamic'])
    diff_static = get_test_intersection(retest_all_report['retest_all_different'], static_report['static'])

    if commit not in tests_selected_dynamic[repository]:
        tests_selected_dynamic[repository][commit] = []
    if commit not in tests_selected_static[repository]:
        tests_selected_static[repository][commit] = []

    for test in diff_dynamic:
        tests_selected_dynamic[repository][commit].append(test)

    for test in diff_static:
        tests_selected_static[repository][commit].append(test)

engine = create_engine(url_git)
with engine.connect() as conn:
    conn.execute(table_ddl)

    for repository in tests_selected_static:
        for commit in tests_selected_dynamic[repository]:
            for test in tests_selected_dynamic[repository][commit]:
                statement = f"INSERT INTO public.\"AnalysisDifferentSelected\" (repo_id, commit, test_name, algorithm) VALUES ({repository}, {commit}, '{test}', 'dynamic')"
                conn.execute(statement)
        for commit in tests_selected_static[repository]:
            for test in tests_selected_static[repository][commit]:
                statement = f"INSERT INTO public.\"AnalysisDifferentSelected\" (repo_id, commit, test_name, algorithm) VALUES ({repository}, {commit}, '{test}', 'static')"
                conn.execute(statement)
