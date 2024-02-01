import sqlalchemy
from sqlalchemy import text, select
from sqlalchemy_utils import create_view

import sqlalchemy as sa

def get_view_definition(path):
    file = open(path, "r")
    content = file.read()
    return text(content)