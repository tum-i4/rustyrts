import pandas as pd

url_mutants = 'postgresql://postgres:rustyrts@localhost:5432/mutants'
url_git = 'postgresql://postgres:rustyrts@localhost:5432/git'


def get_labels_mutants():
    df_labels = pd.read_sql(
        '''
        SELECT r.path, count(m.descr) as number_mutants
        FROM public."Repository" r, public."Commit" c, mutant_extended m
        WHERE r.id = c.repo_id AND c.id = m.commit GROUP BY r.path
        ''',
        url_mutants)

    labels = []
    for row in df_labels.to_dict(orient='records'):
        labels.append(row['path'][row['path'].rfind('/') + 1:] + "\n(" + str(row["number_mutants"]) + ")")

    return labels
