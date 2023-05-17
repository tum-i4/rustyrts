import pandas as pd

from plotter import boxplot, get_labels_mutants, url_mutants

y_label = 'Failed tests, that have been selected'
file = 'failed_and_selected_relative.pdf'

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

not_selected_dynamic = []
not_selected_static = []

failed_retest_all = df_failed_retest_all.to_dict(orient='records')
selected_dynamic = df_selected_dynamic.to_dict(orient='records')
selected_static = df_selected_static.to_dict(orient='records')

assert len(failed_retest_all) == len(selected_dynamic) and len(failed_retest_all) == len(selected_static)

for (retest_all_mutant, dynamic_mutant, static_mutant) in zip(failed_retest_all, selected_dynamic, selected_static):
    repository = retest_all_mutant['repository']
    descr = retest_all_mutant['mutant']

    diff_dynamic = get_test_diff(retest_all_mutant['retest_all_failed'], dynamic_mutant['dynamic'])
    diff_static = get_test_diff(retest_all_mutant['retest_all_failed'], static_mutant['static'])

    num_failed_tests = len(retest_all_mutant['retest_all_failed'].splitlines())
    not_selected_dynamic.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'dynamic',
         'y': 100.0 - len(diff_dynamic) * 100.0 / num_failed_tests if num_failed_tests != 0 else 100.0})
    not_selected_static.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'static',
         'y': 100.0 - len(diff_static) * 100.0 / num_failed_tests if num_failed_tests != 0 else 100.0})

df_failed_retest_all = pd.DataFrame(failed_retest_all)
df_not_selected_dynamic = pd.DataFrame(not_selected_dynamic)
df_not_selected_static = pd.DataFrame(not_selected_static)

df = pd.concat([df_not_selected_dynamic[['repository', 'algorithm', 'y']],
                df_not_selected_static[['repository', 'algorithm', 'y']]])

boxplot(df, labels, y_label, file, ["#E37222", "#A2AD00"], hue='algorithm')
