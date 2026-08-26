#include <QCoreApplication>
#include <QDebug>
#include "qextserialenumerator.h"
int main(int argc, char **argv)
{
    QCoreApplication app(argc, argv);
    const QList<QextPortInfo> ports = QextSerialEnumerator::getPorts();
    for (int i = 0; i < ports.size(); ++i)
        qDebug() << ports.at(i).portName;
    return ports.isEmpty() ? 1 : 0;
}
