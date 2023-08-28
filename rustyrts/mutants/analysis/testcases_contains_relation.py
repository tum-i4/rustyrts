import pandas as pd
from sqlalchemy import create_engine

from rustyrts.mutants.plots._scripts.labels import get_labels_mutants, url_mutants

table_ddl = """
create sequence "AnalysisTestsNotContained_id_seq"
    as integer;

alter sequence "AnalysisTestsNotContained_id_seq" owner to postgres;

create table "AnalysisTestsNotContained"
(
    id                 integer default nextval('"AnalysisTestsNotContained_id_seq"'::regclass) not null
        constraint "AnalysisTestsNotContained_pk"
            primary key,
    commit             integer                                                                not null
        constraint "AnalysisTestsNotContained_Commit_null_fk"
            references "Commit",
    test_name          varchar                                                                not null,
    not_contained_count integer                                                                not null,
    reason             varchar,
    comment            varchar
);

alter table "AnalysisTestsNotContained"
    owner to postgres;

"""


labels = get_labels_mutants()


def get_test_diff(retest_all, other):
    retest_all_tests = retest_all.splitlines()
    other_tests = other.splitlines()
    return list(set(retest_all_tests) - set(other_tests))


df_selected_dynamic = pd.read_sql(
    'SELECT commit as repository, retest_all_mutant_id, dynamic, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
    url_mutants)

df_selected_static = pd.read_sql(
    'SELECT commit as repository, retest_all_mutant_id, static, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
    url_mutants)

not_selected_static = {}

for i in range(1, len(labels) + 1):
    not_selected_static[i] = {}

selected_dynamic = df_selected_dynamic.to_dict(orient='records')
selected_static = df_selected_static.to_dict(orient='records')

assert len(selected_static) == len(selected_static)

for (dynamic_mutant, static_mutant) in zip(selected_dynamic, selected_static):
    assert dynamic_mutant['retest_all_mutant_id'] == static_mutant['retest_all_mutant_id']

    repository = static_mutant['repository']
    descr = static_mutant['mutant']

    diff = get_test_diff(dynamic_mutant['dynamic'], static_mutant['static'])

    for test in diff:
        if test not in not_selected_static[repository]:
            not_selected_static[repository][test] = 1
        else:
            not_selected_static[repository][test] += 1


engine = create_engine(url_mutants)
with engine.connect() as conn:
    conn.execute(table_ddl)

    for commit in not_selected_static:
        for (test, count) in not_selected_static[commit].items():
            statement = f"INSERT INTO public.\"AnalysisTestsNotContained\" (commit, test_name, not_contained_count) VALUES ({commit}, '{test}', {count})"
            conn.execute(statement)