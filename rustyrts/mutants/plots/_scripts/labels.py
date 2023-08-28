import pandas as pd

url_mutants = 'postgresql://postgres:rustyrts@localhost:5432/mutants_final' # TODO: adapt this
output_format = ".svg"


def get_labels_mutants(count=True):
    df_labels = pd.read_sql(
        '''
        SELECT r.path, count(distinct m.descr) as number_mutants
        FROM public."Repository" r, public."Commit" c, mutant_testcase_overview m
        WHERE r.id = c.repo_id AND c.id = m.commit
        GROUP BY c.id, r.path
        ORDER BY c.id
        ''',
        url_mutants)

    labels = []
    for row in df_labels.to_dict(orient='records'):
        if count:
            labels.append(row['path'][row['path'].rfind('/') + 1:] + "\n(" + str(row["number_mutants"]) + ")")
        else:
            labels.append(row['path'][row['path'].rfind('/') + 1:])

    padding = max(len(label.splitlines()[0]) for label in labels)
    for i in range(len(labels)):
        lines = []
        first_line = labels[i].splitlines()[0]
        for line in labels[i].splitlines():
            lines.append((padding - len(first_line)) * "  " + line)
        labels[i] = '\n'.join(lines)

    return labels
