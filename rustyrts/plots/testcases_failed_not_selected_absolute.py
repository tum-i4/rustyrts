import pandas as pd
from sqlalchemy import create_engine

from plotter import boxplot, get_labels_mutants, url_mutants

table_ddl = """
create sequence "AnalysisTestsNotSelected_id_seq"
    as integer;

alter sequence "AnalysisTestsNotSelected_id_seq" owner to postgres;

create table "AnalysisTestsNotSelected"
(
    id                 integer default nextval('"AnalysisTestsNotSelected_id_seq"'::regclass) not null
        constraint "AnalysisTestsNotSelected_pk"
            primary key,
    commit             integer                                                                not null
        constraint "AnalysisTestsNotSelected_Commit_null_fk"
            references "Commit",
    test_name          varchar                                                                not null,
    not_selected_count integer                                                                not null,
    reason             varchar,
    comment            varchar,
    algorithm          varchar                                                                not null
);

alter table "AnalysisTestsNotSelected"
    owner to postgres;

"""

y_label = 'Failed tests, not selected'
file = 'failed_but_not_selected_absolute.pdf'

labels = get_labels_mutants()


def get_test_diff(retest_all, other):
    retest_all_tests = retest_all.splitlines()
    other_tests = other.splitlines()
    return list(set(retest_all_tests) - set(other_tests))


df_failed_retest_all = pd.read_sql(
    'SELECT commit as repository, retest_all_failed , descr as mutant FROM testcases_failed WHERE descr != \'baseline\' ORDER BY commit, descr',
    url_mutants)

df_selected_dynamic = pd.read_sql(
    'SELECT commit as repository, dynamic, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
    url_mutants)

df_selected_static = pd.read_sql(
    'SELECT commit as repository, static, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
    url_mutants)

should_be_selected = []
not_selected_dynamic = []
not_selected_static = []

not_selected_dynamic_summary = {}
not_selected_static_summary = {}

for i in range(1, len(labels) + 1):
    not_selected_dynamic_summary[i] = {}
    not_selected_static_summary[i] = {}

failed_retest_all = df_failed_retest_all.to_dict(orient='records')
selected_dynamic = df_selected_dynamic.to_dict(orient='records')
selected_static = df_selected_static.to_dict(orient='records')

assert len(failed_retest_all) == len(selected_dynamic) and len(failed_retest_all) == len(selected_static)

for (retest_all_mutant, dynamic_mutant, static_mutant) in zip(failed_retest_all, selected_dynamic, selected_static):
    repository = retest_all_mutant['repository']
    descr = retest_all_mutant['mutant']

    diff_dynamic = get_test_diff(retest_all_mutant['retest_all_failed'], dynamic_mutant['dynamic'])
    diff_static = get_test_diff(retest_all_mutant['retest_all_failed'], static_mutant['static'])

    for test in diff_dynamic:
        if test not in not_selected_dynamic_summary[repository]:
            not_selected_dynamic_summary[repository][test] = 1
        else:
            not_selected_dynamic_summary[repository][test] += 1

    for test in diff_static:
        if test not in not_selected_static_summary[repository]:
            not_selected_static_summary[repository][test] = 1
        else:
            not_selected_static_summary[repository][test] += 1

    should_be_selected.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'retest-all',
         'y': len(retest_all_mutant['retest_all_failed'].splitlines())})
    not_selected_dynamic.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'dynamic', 'y': len(diff_dynamic)})
    not_selected_static.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'static', 'y': len(diff_static)})

into_db = False
if not into_db:
    for i in range(1, len(labels) + 1):
        print("Repository " + str(i))
        print("    Dynamic: " + str(not_selected_dynamic_summary[i]))
        print("    Static: " + str(not_selected_static_summary[i]))
        print('\n')
else:
    engine = create_engine(url_mutants)
    with engine.connect() as conn:
        conn.execute(table_ddl)

        for commit in not_selected_dynamic_summary:
            for (test, count) in not_selected_dynamic_summary[commit].items():
                statement = f"INSERT INTO public.\"AnalysisTestsNotSelected\" (commit, test_name, not_selected_count, algorithm) VALUES ({commit}, '{test}', {count}, 'dynamic')"
                conn.execute(statement)
        for commit in not_selected_static_summary:
            for (test, count) in not_selected_static_summary[commit].items():
                statement = f"INSERT INTO public.\"AnalysisTestsNotSelected\" (commit, test_name, not_selected_count, algorithm) VALUES ({commit}, '{test}', {count}, 'static')"
                conn.execute(statement)


df_failed_retest_all = pd.DataFrame(should_be_selected)
df_not_selected_dynamic = pd.DataFrame(not_selected_dynamic)
df_not_selected_static = pd.DataFrame(not_selected_static)

df = pd.concat([df_failed_retest_all[['repository', 'algorithm', 'y']],
                df_not_selected_dynamic[['repository', 'algorithm', 'y']],
                df_not_selected_static[['repository', 'algorithm', 'y']]])

boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"], hue='algorithm')
