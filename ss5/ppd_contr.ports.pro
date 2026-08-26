#-------------------------------------------------
#
# Project created by QtCreator 2016-06-22T10:29:57
#
#-------------------------------------------------

#QT       += core gui xml network sql
#greaterThan(QT_MAJOR_VERSION, 4): QT += widgets
QT       +=  xml network sql
TARGET = h750
TEMPLATE = app

win32 {
    DEFINES += _TTY_WIN_
    LIBS += setupapi.lib advapi32.lib
}


SOURCES += main.cpp\
    ex.cpp \
    ser.cpp \
    module_programming.cpp \
    ud.cpp \
    ob.cpp \
    qextserialbase.cpp \
    qextserialenumerator.cpp \
    qextserialport.cpp \
    win_qextserialport.cpp

HEADERS  += ex.h \
    ob.h \
    ser.h \
    module_programming.h \
    ud.h \
    qextserialbase.h \
    qextserialenumerator.h \
    qextserialport.h \
    win_qextserialport.h

FORMS    += ex.ui \
    ob.ui
