import pandas as pd

from rustyrts.mutants.plots.scripts.labels import get_labels_mutants, url_mutants, output_format
from rustyrts.util.plotter import boxplot

y_label = 'Tests that have been selected'
file = '../contains_all_tests' + output_format

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

not_selected_static = []

selected_dynamic = df_selected_dynamic.to_dict(orient='records')
selected_static = df_selected_static.to_dict(orient='records')

assert len(selected_static) == len(selected_static)

for (dynamic_mutant, static_mutant) in zip(selected_dynamic, selected_static):
    assert dynamic_mutant['retest_all_mutant_id'] == static_mutant['retest_all_mutant_id']

    repository = static_mutant['repository']
    descr = static_mutant['mutant']

    diff = get_test_diff(dynamic_mutant['dynamic'], static_mutant['static'])

    not_selected_static.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'dynamic but not static', 'y': len(diff)})

df_not_selected_static = pd.DataFrame(not_selected_static)

df = pd.concat([df_not_selected_static[['repository', 'algorithm', 'y']]])

boxplot(df, labels, y_label, file, ["#E37222"], hue='algorithm')
