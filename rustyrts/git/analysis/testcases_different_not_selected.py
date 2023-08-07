import pandas as pd
from sqlalchemy import create_engine

from rustyrts.git.plots.scripts.labels import get_labels_git, url_git

table_ddl = """
create sequence "AnalysisDifferentNotSelected_id_seq"
    as integer;

alter sequence "AnalysisDifferentNotSelected_id_seq" owner to postgres;

create table "AnalysisDifferentNotSelected"
(
    id                 integer default nextval('"AnalysisDifferentNotSelected_id_seq"'::regclass) not null
        constraint "AnalysisDifferentNotSelected_pk"
            primary key,
    repo_id            integer                                                                not null
        constraint "AnalysisTestsNotContained_Repoistory_null_fk"
            references "Repository",
    commit             integer                                                                not null
        constraint "AnalysisDifferentNotSelected_Commit_null_fk"
            references "Commit",
    test_name          varchar                                                                not null,
    parent_result      teststatus,
    result         teststatus,
    reason             varchar,
    comment            varchar,
    algorithm          varchar                                                                not null
);

alter table "AnalysisDifferentNotSelected"
    owner to postgres;

"""

labels = get_labels_git()


def get_test_diff(retest_all, other):
    retest_all_tests = retest_all.splitlines()
    other_tests = other.splitlines()
    return list(set(retest_all_tests) - set(other_tests))


df_failed_retest_all = pd.read_sql(
    'SELECT c.repo_id as repository, commit, retest_all_different FROM testcases_newly_different join "Commit" c ON c.id = commit ORDER BY commit',
    url_git)

df_selected_dynamic = pd.read_sql(
    'SELECT c.repo_id as repository, commit, dynamic FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
    url_git)

df_selected_static = pd.read_sql(
    'SELECT c.repo_id as repository, commit, static FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
    url_git)

not_selected_dynamic = {}
not_selected_static = {}

for i in range(1, len(labels) + 1):
    not_selected_dynamic[i] = {}
    not_selected_static[i] = {}

failed_retest_all = df_failed_retest_all.to_dict(orient='records')
selected_dynamic = df_selected_dynamic.to_dict(orient='records')
selected_static = df_selected_static.to_dict(orient='records')

assert len(failed_retest_all) == len(selected_dynamic) and len(failed_retest_all) == len(selected_static)

for (retest_all_report, dynamic_report, static_report) in zip(failed_retest_all, selected_dynamic, selected_static):
    assert retest_all_report['commit'] == dynamic_report['commit']
    assert retest_all_report['commit'] == static_report['commit']

    repository = retest_all_report['repository']
    commit = retest_all_report['commit']

    diff_dynamic = get_test_diff(retest_all_report['retest_all_different'], dynamic_report['dynamic'])
    diff_static = get_test_diff(retest_all_report['retest_all_different'], static_report['static'])

    if commit not in not_selected_dynamic[repository]:
        not_selected_dynamic[repository][commit] = []
    if commit not in not_selected_static[repository]:
        not_selected_static[repository][commit] = []

    for test in diff_dynamic:
        not_selected_dynamic[repository][commit].append(test)

    for test in diff_static:
        not_selected_static[repository][commit].append(test)

engine = create_engine(url_git)
with engine.connect() as conn:
    conn.execute(table_ddl)

    for repository in not_selected_static:
        for commit in not_selected_dynamic[repository]:
            for test in not_selected_dynamic[repository][commit]:
                statement = f"INSERT INTO public.\"AnalysisDifferentNotSelected\" (repo_id, commit, test_name, algorithm) VALUES ({repository}, {commit}, '{test}', 'dynamic')"
                conn.execute(statement)
        for commit in not_selected_static[repository]:
            for test in not_selected_static[repository][commit]:
                statement = f"INSERT INTO public.\"AnalysisDifferentNotSelected\" (repo_id, commit, test_name, algorithm) VALUES ({repository}, {commit}, '{test}', 'static')"
                conn.execute(statement)
